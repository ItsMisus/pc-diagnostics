//! HID LampArray devices on macOS through IOHIDManager, plus the list of devices of known
//! RGB vendors without LampArray (recognised, not controllable).
//!
//! Enumeration only copies the device list (no device is opened for that, so no Input
//! Monitoring prompt for ordinary keyboards). A LampArray device is opened (shared, not
//! seized) to read `LampArrayAttributesReport` and each lamp's `LampAttributesResponseReport`;
//! colors go out as `LampMultiUpdateReport`s, and `LampArrayControlReport` turns autonomous
//! mode off while the app drives the lamps and back on when it lets go. The report layouts
//! come from the device's own feature elements (`lamparray.rs`).
//!
//! Not tested on hardware: no LampArray device was available while writing this. The
//! report encoding is unit-tested against Microsoft's reference descriptor.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Mutex;

use models::control::Support;
use models::control::lighting::{LightingColor, LightingDevice, LightingSnapshot, LightingSource};
use models::settings::Language;
use models::{AppError, ErrorCode};
use objc2_core_foundation::{CFDictionary, CFIndex, CFNumber, CFRetained, CFString};
use objc2_io_kit::{
    IOHIDDevice, IOHIDElement, IOHIDElementType, IOHIDManager, IOHIDReportType,
    kIOHIDOptionsTypeNone, kIOReturnSuccess,
};

use super::lamparray::{
    self, ArrayAttributes, Field, LampArrayReports, LampAttributes, LampValue, ReportLayout,
};
use super::vendors::{kind_from_lamp_array_kind, kind_from_name, known_rgb_vendor, vendor_device};
use super::{DeviceLayout, LightingFrame, normalize_positions};

const PREFIX: &str = "hid:";
const VENDOR_PREFIX: &str = "vendor:";
const APPLE: u32 = 0x05ac;
/// `kIOReturnNotPermitted`: TCC (Input Monitoring) refused the open.
const NOT_PERMITTED: i32 = 0xE00002E2_u32 as i32;
/// Lamps read per device at most (a full keyboard has ~110).
const MAX_LAMPS: u32 = 512;

pub fn owns(id: &str) -> bool {
    id.starts_with(PREFIX)
}

struct LampArrayDevice {
    device: CFRetained<IOHIDDevice>,
    reports: LampArrayReports,
    attributes: ArrayAttributes,
    lamps: Vec<LampAttributes>,
    positions: Vec<f64>,
    controlling: bool,
}

#[derive(Default)]
struct Inner {
    arrays: HashMap<String, LampArrayDevice>,
}

/// Serialized access to the HID objects.
struct Shared(Mutex<Inner>);
// SAFETY: the IOHIDDevice references are only used synchronously (no run loop, no
// callbacks) and always under the mutex, one thread at a time.
unsafe impl Send for Shared {}
// SAFETY: as above; `&Shared` only reaches the devices through the mutex.
unsafe impl Sync for Shared {}

pub struct HidLighting {
    shared: Shared,
}

fn number(n: u32) -> CFRetained<CFNumber> {
    CFNumber::new_i32(n as i32)
}

fn prop_u32(dev: &IOHIDDevice, key: &'static str) -> Option<u32> {
    let v = dev.property(&CFString::from_static_str(key))?;
    v.downcast::<CFNumber>().ok()?.as_i64().map(|n| n as u32)
}

fn prop_string(dev: &IOHIDDevice, key: &'static str) -> Option<String> {
    let v = dev.property(&CFString::from_static_str(key))?;
    let s = v.downcast::<CFString>().ok()?.to_string();
    let s = s.trim().to_owned();
    (!s.is_empty()).then_some(s)
}

/// Devices matching `matching` (all HID devices with `None`), without opening them.
fn copy_devices(matching: Option<&CFDictionary>) -> Vec<CFRetained<IOHIDDevice>> {
    let manager = IOHIDManager::new(None, kIOHIDOptionsTypeNone);
    // SAFETY: `matching` is a valid CFDictionary of CFString keys and CFNumber values.
    unsafe { manager.set_device_matching(matching) };
    let Some(set) = manager.devices() else {
        return Vec::new();
    };
    let count = set.count().max(0) as usize;
    let mut raw: Vec<*const c_void> = vec![std::ptr::null(); count];
    // SAFETY: `raw` has room for `count` values; the set holds IOHIDDevice references that
    // stay alive while `set` does, and each is retained below before `set` is released.
    unsafe { set.values(raw.as_mut_ptr()) };
    raw.into_iter()
        .filter_map(|p| NonNull::new(p as *mut IOHIDDevice))
        // SAFETY: a valid IOHIDDevice from the set; `retain` adds our own reference.
        .map(|p| unsafe { CFRetained::retain(p) })
        .collect()
}

fn lamp_array_matching() -> CFRetained<CFDictionary<CFString, CFNumber>> {
    let page = CFString::from_static_str("DeviceUsagePage");
    let usage = CFString::from_static_str("DeviceUsage");
    let p = number(lamparray::USAGE_PAGE);
    let u = number(lamparray::usage::LAMP_ARRAY);
    CFDictionary::from_slices(&[&*page, &*usage], &[&*p, &*u])
}

/// Stable id from vendor, product and USB location (or serial).
fn device_id(dev: &IOHIDDevice, vid: u32, pid: u32) -> String {
    let loc = prop_u32(dev, "LocationID")
        .map(|l| format!("{l:x}"))
        .or_else(|| prop_string(dev, "SerialNumber"))
        .unwrap_or_else(|| "0".to_owned());
    format!("{PREFIX}{vid:04x}:{pid:04x}:{loc}")
}

/// Feature report layouts from the device's elements, in descriptor (cookie) order.
fn feature_layouts(dev: &IOHIDDevice) -> Vec<ReportLayout> {
    // SAFETY: no matching dictionary (all elements), default options.
    let Some(arr) = (unsafe { dev.matching_elements(None, kIOHIDOptionsTypeNone) }) else {
        return Vec::new();
    };
    // SAFETY: the array holds IOHIDElement references (documented return type).
    let arr = unsafe { arr.cast_unchecked::<IOHIDElement>() };
    let mut elements: Vec<(u32, u32, Field)> = arr
        .iter()
        .filter(|e| e.r#type() == IOHIDElementType::Feature)
        .map(|e| {
            let usage = if e.usage_page() == lamparray::USAGE_PAGE {
                e.usage()
            } else {
                0
            };
            (
                e.report_id(),
                e.cookie(),
                Field::new(usage, e.report_size(), e.report_count().max(1)),
            )
        })
        .collect();
    elements.sort_by_key(|(id, cookie, _)| (*id, *cookie));
    let mut by_id: Vec<(u32, Vec<Field>)> = Vec::new();
    for (id, _, f) in elements {
        match by_id.last_mut() {
            Some((last, fields)) if *last == id => fields.push(f),
            _ => by_id.push((id, vec![f])),
        }
    }
    by_id
        .into_iter()
        .filter(|(id, _)| *id <= 0xff)
        .map(|(id, fields)| ReportLayout::new(id as u8, &fields))
        .collect()
}

fn get_feature(dev: &IOHIDDevice, layout: &ReportLayout) -> Option<Vec<u8>> {
    let mut buf = layout.buffer();
    let mut len = buf.len() as CFIndex;
    let ptr = NonNull::new(buf.as_mut_ptr())?;
    // SAFETY: `buf` has `len` writable bytes; `len` is a valid in/out pointer.
    let r = unsafe {
        dev.report(
            IOHIDReportType::Feature,
            CFIndex::from(layout.report_id),
            ptr,
            NonNull::from(&mut len),
        )
    };
    (r == kIOReturnSuccess).then_some(buf)
}

fn set_feature(dev: &IOHIDDevice, report_id: u8, buf: &mut [u8]) -> Result<(), AppError> {
    let ptr =
        NonNull::new(buf.as_mut_ptr()).ok_or_else(|| AppError::internal("empty HID report"))?;
    // SAFETY: `buf` holds the whole report (report id first when non-zero).
    let r = unsafe {
        dev.set_report(
            IOHIDReportType::Feature,
            CFIndex::from(report_id),
            ptr,
            buf.len() as CFIndex,
        )
    };
    if r == kIOReturnSuccess {
        Ok(())
    } else {
        Err(
            AppError::new(ErrorCode::Platform, "HID feature report refused")
                .with_detail(format!("IOReturn 0x{:08x}", r as u32)),
        )
    }
}

/// Opens a LampArray and reads its attributes and lamps.
fn read_lamp_array(dev: CFRetained<IOHIDDevice>) -> Result<LampArrayDevice, i32> {
    let r = dev.open(kIOHIDOptionsTypeNone);
    if r != kIOReturnSuccess {
        return Err(r);
    }
    let fail = |dev: &IOHIDDevice| {
        dev.close(kIOHIDOptionsTypeNone);
        Err(-1)
    };
    let Some(reports) = LampArrayReports::identify(feature_layouts(&dev)) else {
        return fail(&dev);
    };
    let Some(attributes) = get_feature(&dev, &reports.attributes)
        .and_then(|b| lamparray::decode_attributes(&reports, &b))
    else {
        return fail(&dev);
    };
    let mut lamps = Vec::new();
    for id in 0..attributes.lamp_count.min(MAX_LAMPS) {
        let mut req = lamparray::encode_attributes_request(&reports, id);
        if set_feature(&dev, reports.attributes_request.report_id, &mut req).is_err() {
            break;
        }
        match get_feature(&dev, &reports.attributes_response)
            .and_then(|b| lamparray::decode_lamp(&reports, &b))
        {
            Some(lamp) => lamps.push(lamp),
            None => break,
        }
    }
    if lamps.is_empty() {
        return fail(&dev);
    }
    let xs: Vec<f64> = lamps.iter().map(|l| f64::from(l.x)).collect();
    Ok(LampArrayDevice {
        positions: normalize_positions(&xs),
        device: dev,
        reports,
        attributes,
        lamps,
        controlling: false,
    })
}

fn lamp_value(lamp: &LampAttributes, c: LightingColor) -> LampValue {
    let (red, green, blue, intensity) = if lamp.color_capable() {
        (
            lamparray::scale_channel(c.r, lamp.red_levels),
            lamparray::scale_channel(c.g, lamp.green_levels),
            lamparray::scale_channel(c.b, lamp.blue_levels),
            lamp.intensity_levels.saturating_sub(1).max(1),
        )
    } else {
        let level = c.r.max(c.g).max(c.b);
        (
            0,
            0,
            0,
            lamparray::scale_channel(level, lamp.intensity_levels.max(2)),
        )
    };
    LampValue {
        id: lamp.id,
        red,
        green,
        blue,
        intensity,
    }
}

impl LampArrayDevice {
    fn set_autonomous(&mut self, on: bool) -> Result<(), AppError> {
        if let Some(mut b) = lamparray::encode_control(&self.reports, on) {
            let id = self.reports.control.as_ref().map_or(0, |l| l.report_id);
            set_feature(&self.device, id, &mut b)?;
        }
        self.controlling = !on;
        Ok(())
    }

    fn write(&mut self, frame: &LightingFrame) -> Result<(), AppError> {
        if !self.controlling {
            self.set_autonomous(false)?;
        }
        let values: Vec<LampValue> = match frame {
            LightingFrame::Colors(colors) => self
                .lamps
                .iter()
                .enumerate()
                .map(|(i, lamp)| {
                    let c = colors.get(i).or(colors.last()).copied().unwrap_or_default();
                    lamp_value(lamp, c)
                })
                .collect(),
            LightingFrame::Level(l) => {
                let v = (l.clamp(0.0, 1.0) * 255.0).round() as u8;
                self.lamps
                    .iter()
                    .map(|lamp| lamp_value(lamp, LightingColor::rgb(v, v, v)))
                    .collect()
            }
        };
        // One color for every lamp: a single range update when the device has one.
        let uniform = values
            .windows(2)
            .all(|w| (w[0].red, w[0].green, w[0].blue) == (w[1].red, w[1].green, w[1].blue));
        if uniform
            && let (Some(first), Some(range)) = (values.first(), self.reports.range_update.as_ref())
        {
            let start = self.lamps.first().map_or(0, |l| l.id);
            let end = self.lamps.last().map_or(0, |l| l.id);
            if let Some(mut b) = lamparray::encode_range_update(&self.reports, start, end, *first) {
                return set_feature(&self.device, range.report_id, &mut b);
            }
        }
        let id = self.reports.multi_update.report_id;
        for mut b in lamparray::encode_multi_update(&self.reports, &values) {
            set_feature(&self.device, id, &mut b)?;
        }
        Ok(())
    }
}

impl Drop for LampArrayDevice {
    fn drop(&mut self) {
        if self.controlling {
            let _ = self.set_autonomous(true);
        }
        self.device.close(kIOHIDOptionsTypeNone);
    }
}

impl HidLighting {
    pub fn new() -> Self {
        Self {
            shared: Shared(Mutex::new(Inner::default())),
        }
    }

    fn inner(&self) -> Result<std::sync::MutexGuard<'_, Inner>, AppError> {
        self.shared
            .0
            .lock()
            .map_err(|_| AppError::internal("HID lighting lock poisoned"))
    }

    pub fn devices(&self, lang: Language) -> Vec<LightingDevice> {
        let Ok(mut inner) = self.inner() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut lamp_vid_pid = HashSet::new();
        let matching = lamp_array_matching();
        let mut seen = HashSet::new();
        for dev in copy_devices(Some(matching.as_opaque())) {
            let vid = prop_u32(&dev, "VendorID").unwrap_or(0);
            let pid = prop_u32(&dev, "ProductID").unwrap_or(0);
            let id = device_id(&dev, vid, pid);
            if !seen.insert(id.clone()) {
                continue;
            }
            lamp_vid_pid.insert((vid, pid));
            let name = prop_string(&dev, "Product").unwrap_or_else(|| {
                lang.pick("Dispositivo LampArray", "LampArray device")
                    .to_owned()
            });
            let vendor = prop_string(&dev, "Manufacturer")
                .or_else(|| known_rgb_vendor(vid).map(str::to_owned));
            // Keep a device already open (and possibly driven) as it is.
            if !inner.arrays.contains_key(&id) {
                match read_lamp_array(dev) {
                    Ok(a) => {
                        inner.arrays.insert(id.clone(), a);
                    }
                    Err(code) => {
                        let reason = if code == NOT_PERMITTED {
                            lang.pick(
                                "macOS non permette di aprire il dispositivo: consenti PC Diagnostics in Impostazioni di Sistema > Privacy e sicurezza > Monitoraggio input.",
                                "macOS does not let the app open the device: allow PC Diagnostics in System Settings > Privacy & Security > Input Monitoring.",
                            )
                        } else {
                            lang.pick(
                                "Il dispositivo dichiara LampArray ma non ha risposto alle richieste standard.",
                                "The device declares LampArray but did not answer the standard requests.",
                            )
                        };
                        out.push(LightingDevice {
                            id,
                            kind: kind_from_name(&name),
                            name,
                            vendor,
                            lamp_count: 0,
                            color_capable: false,
                            color: Support::unsupported(reason),
                            brightness: Support::unsupported(reason),
                            effects: Support::unsupported(reason),
                            source: LightingSource::LampArray,
                        });
                        continue;
                    }
                }
            }
            if let Some(a) = inner.arrays.get(&id) {
                let color_capable = a.lamps.iter().any(LampAttributes::color_capable);
                out.push(LightingDevice {
                    id,
                    kind: kind_from_lamp_array_kind(a.attributes.kind, &name),
                    name,
                    vendor,
                    lamp_count: a.lamps.len() as u32,
                    color_capable,
                    color: if color_capable {
                        Support::supported()
                    } else {
                        Support::unsupported(lang.pick(
                            "Le luci di questo dispositivo hanno un solo colore.",
                            "This device's lights have a single color.",
                        ))
                    },
                    brightness: Support::supported(),
                    effects: Support::supported(),
                    source: LightingSource::LampArray,
                });
            }
        }
        // Forget devices that went away (dropping hands them back).
        let present: HashSet<&String> = out.iter().map(|d| &d.id).collect();
        inner.arrays.retain(|id, _| present.contains(id));

        // Recognised vendors without LampArray: one entry per product.
        let mut products = HashSet::new();
        for dev in copy_devices(None) {
            let vid = prop_u32(&dev, "VendorID").unwrap_or(0);
            let pid = prop_u32(&dev, "ProductID").unwrap_or(0);
            if vid == APPLE || lamp_vid_pid.contains(&(vid, pid)) || !products.insert((vid, pid)) {
                continue;
            }
            let Some(vendor) = known_rgb_vendor(vid) else {
                continue;
            };
            let name = prop_string(&dev, "Product").unwrap_or_else(|| vendor.to_owned());
            let kind = match (
                prop_u32(&dev, "PrimaryUsagePage"),
                prop_u32(&dev, "PrimaryUsage"),
            ) {
                (Some(1), Some(6)) => models::control::lighting::LightingDeviceKind::Keyboard,
                (Some(1), Some(2)) => models::control::lighting::LightingDeviceKind::Mouse,
                _ => kind_from_name(&name),
            };
            out.push(vendor_device(
                format!("{VENDOR_PREFIX}{vid:04x}:{pid:04x}"),
                name,
                prop_string(&dev, "Manufacturer")
                    .as_deref()
                    .unwrap_or(vendor),
                kind,
                lang,
            ));
        }
        out
    }

    pub fn layout(&self, id: &str) -> DeviceLayout {
        self.inner()
            .ok()
            .and_then(|i| {
                i.arrays.get(id).map(|a| DeviceLayout {
                    lamp_count: a.lamps.len() as u32,
                    positions: a.positions.clone(),
                })
            })
            .unwrap_or_default()
    }

    /// LampArray colors cannot be read back: what the device shows on its own is its
    /// autonomous mode, which is what a restore brings back.
    pub fn snapshot(&self, _id: &str) -> LightingSnapshot {
        LightingSnapshot::LampArray {
            autonomous: true,
            enabled: None,
            brightness: None,
        }
    }

    pub fn take_control(&self, id: &str) -> Result<(), AppError> {
        let mut inner = self.inner()?;
        let a = inner.arrays.get_mut(id).ok_or_else(|| missing(id))?;
        a.set_autonomous(false)
    }

    pub fn write(&self, id: &str, frame: &LightingFrame) -> Result<(), AppError> {
        let mut inner = self.inner()?;
        let a = inner.arrays.get_mut(id).ok_or_else(|| missing(id))?;
        a.write(frame)
    }

    pub fn restore(&self, id: &str, snapshot: &LightingSnapshot) -> Result<(), AppError> {
        let autonomous = match snapshot {
            LightingSnapshot::LampArray { autonomous, .. } => *autonomous,
            LightingSnapshot::Backlight { .. } => true,
        };
        let mut inner = self.inner()?;
        let a = inner.arrays.get_mut(id).ok_or_else(|| missing(id))?;
        if autonomous {
            a.set_autonomous(true)
        } else {
            Ok(())
        }
    }

    pub fn release(&self, id: &str) {
        if let Ok(mut inner) = self.inner()
            && let Some(a) = inner.arrays.get_mut(id)
            && a.controlling
            && let Err(e) = a.set_autonomous(true)
        {
            tracing::warn!(id, error = %e.message, "could not hand the LampArray back");
        }
    }
}

fn missing(id: &str) -> AppError {
    AppError::new(
        ErrorCode::NotFound,
        format!("LampArray {id} is not connected"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monochrome_lamps_use_the_intensity_channel() {
        let lamp = LampAttributes {
            id: 3,
            x: 0,
            y: 0,
            z: 0,
            red_levels: 0,
            green_levels: 0,
            blue_levels: 0,
            intensity_levels: 256,
            programmable: true,
        };
        let v = lamp_value(&lamp, LightingColor::rgb(10, 128, 0));
        assert_eq!(
            (v.id, v.red, v.green, v.blue, v.intensity),
            (3, 0, 0, 0, 128)
        );
        let rgb = LampAttributes {
            red_levels: 256,
            green_levels: 256,
            blue_levels: 256,
            intensity_levels: 1,
            ..lamp
        };
        let v = lamp_value(&rgb, LightingColor::rgb(10, 128, 0));
        assert_eq!((v.red, v.green, v.blue, v.intensity), (10, 128, 0, 1));
    }

    #[test]
    fn ids_are_prefixed() {
        assert!(owns("hid:1b1c:1b2d:14200000"));
        assert!(!owns("vendor:1b1c:1b2d"));
    }

    /// The HID device list of this Mac can be copied without opening anything.
    #[test]
    fn host_hid_devices_copy() {
        let all = copy_devices(None);
        eprintln!("{} HID devices", all.len());
        let matching = lamp_array_matching();
        let lamps = copy_devices(Some(matching.as_opaque()));
        eprintln!("{} LampArray devices", lamps.len());
    }
}
