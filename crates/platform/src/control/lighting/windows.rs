//! HID LampArray devices on Windows through `Windows.Devices.Lights.LampArray` (Dynamic
//! Lighting, Windows 11 22H2+), plus HID devices of known RGB vendors without LampArray.
//!
//! Windows lets only the foreground app, or the app chosen as the background controller in
//! Settings > Personalization > Dynamic Lighting, drive the lamps (`IsAvailable`). Colors last
//! while the app holds the `LampArray` object; dropping it hands the lamps back to Windows.
//! Compiled from macOS only: not run on Windows hardware yet.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use models::control::Support;
use models::control::lighting::{
    LightingColor, LightingDevice, LightingDeviceKind, LightingSnapshot, LightingSource,
};
use models::settings::Language;
use models::{AppError, ErrorCode};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Devices::Lights::{LampArray, LampArrayKind};
use windows::UI::Color;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::HSTRING;

use super::vendors::{kind_from_name, known_rgb_vendor, vendor_device};
use super::{DeviceLayout, LightingFrame, normalize_positions};

const PREFIX: &str = "win:";
const VENDOR_PREFIX: &str = "vendor:";
/// AQS for enabled HID device interfaces.
const HID_AQS: &str = "System.Devices.InterfaceClassGuid:=\"{4D1E55B2-F16F-11CF-88CB-001111000030}\" AND System.Devices.InterfaceEnabled:=System.StructuredQueryType.Boolean#True";

pub fn owns(id: &str) -> bool {
    id.starts_with(PREFIX)
}

pub fn dynamic_lighting_note(lang: Language) -> &'static str {
    lang.pick(
        "Windows lascia comandare le luci solo all'app in primo piano o a quella scelta in Impostazioni > Personalizzazione > Illuminazione dinamica.",
        "Windows lets only the foreground app, or the one chosen in Settings > Personalization > Dynamic Lighting, control the lights.",
    )
}

/// Joins the multithreaded apartment once per thread (WinRT calls need one).
fn ensure_apartment() {
    thread_local!(static DONE: Cell<bool> = const { Cell::new(false) });
    DONE.with(|d| {
        if !d.get() {
            // SAFETY: plain initialization; an already initialized thread returns an error
            // (S_FALSE / RPC_E_CHANGED_MODE) that is harmless to ignore.
            let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
            d.set(true);
        }
    });
}

fn platform(e: windows::core::Error) -> AppError {
    AppError::new(ErrorCode::Platform, "Windows lighting call failed").with_detail(e.message())
}

struct Held {
    array: LampArray,
    positions: Vec<f64>,
    indices: Vec<i32>,
}

pub struct WinLighting {
    held: Mutex<HashMap<String, Held>>,
}

/// `VID_1532&PID_0084` in a device interface path.
fn vid_pid(path: &str) -> Option<(u32, u32)> {
    let up = path.to_ascii_uppercase();
    let hex = |key: &str| {
        let at = up.find(key)? + key.len();
        u32::from_str_radix(up.get(at..at + 4)?, 16).ok()
    };
    Some((hex("VID_")?, hex("PID_")?))
}

fn kind(k: LampArrayKind, name: &str) -> LightingDeviceKind {
    match k {
        LampArrayKind::Keyboard => LightingDeviceKind::Keyboard,
        LampArrayKind::Mouse => LightingDeviceKind::Mouse,
        LampArrayKind::Chassis => LightingDeviceKind::Case,
        LampArrayKind::Headset => LightingDeviceKind::Headset,
        _ => kind_from_name(name),
    }
}

impl WinLighting {
    pub fn new() -> Self {
        Self {
            held: Mutex::new(HashMap::new()),
        }
    }

    fn held(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Held>>, AppError> {
        self.held
            .lock()
            .map_err(|_| AppError::internal("lighting lock poisoned"))
    }

    fn open(id: &str) -> Result<Held, AppError> {
        let device_id = id.strip_prefix(PREFIX).unwrap_or(id);
        let array = LampArray::FromIdAsync(&HSTRING::from(device_id))
            .map_err(platform)?
            .join()
            .map_err(platform)?;
        let count = array.LampCount().map_err(platform)?.max(0);
        let width = array.BoundingBox().map(|b| b.X).unwrap_or(0.0);
        let xs: Vec<f64> = (0..count)
            .map(|i| {
                array
                    .GetLampInfo(i)
                    .and_then(|l| l.Position())
                    .map(|p| f64::from(p.X))
                    .unwrap_or(0.0)
            })
            .collect();
        let positions = if width > 0.0 {
            xs.iter()
                .map(|x| (x / f64::from(width)).clamp(0.0, 1.0))
                .collect()
        } else {
            normalize_positions(&xs)
        };
        Ok(Held {
            array,
            positions,
            indices: (0..count).collect(),
        })
    }

    pub fn devices(&self, lang: Language) -> Vec<LightingDevice> {
        ensure_apartment();
        let mut out = Vec::new();
        let mut lamp_products = HashSet::new();
        let found = LampArray::GetDeviceSelector()
            .and_then(|s| DeviceInformation::FindAllAsyncAqsFilter(&s))
            .and_then(|op| op.join());
        let Ok(mut held) = self.held() else {
            return out;
        };
        if let Ok(list) = found {
            for info in &list {
                let Ok(raw_id) = info.Id() else { continue };
                let id = format!("{PREFIX}{raw_id}");
                let name = info.Name().map(|n| n.to_string()).unwrap_or_default();
                // Read through the held object if the app drives it already, otherwise
                // through a temporary one (holding a LampArray may claim the lamps).
                let temp;
                let h = match held.get(&id) {
                    Some(h) => h,
                    None => match Self::open(&id) {
                        Ok(h) => {
                            temp = h;
                            &temp
                        }
                        Err(e) => {
                            tracing::warn!(id, error = %e.message, "LampArray did not open");
                            continue;
                        }
                    },
                };
                let a = &h.array;
                let vid = a.HardwareVendorId().unwrap_or(0);
                let pid = a.HardwareProductId().unwrap_or(0);
                lamp_products.insert((u32::from(vid), u32::from(pid)));
                let color_capable = a
                    .GetLampInfo(0)
                    .and_then(|l| {
                        Ok(l.RedLevelCount()? > 1
                            && l.GreenLevelCount()? > 1
                            && l.BlueLevelCount()? > 1)
                    })
                    .unwrap_or(true);
                let available = a.IsAvailable().unwrap_or(true);
                let control = if available {
                    Support::supported()
                } else {
                    Support::unsupported(dynamic_lighting_note(lang))
                };
                out.push(LightingDevice {
                    kind: kind(a.LampArrayKind().unwrap_or(LampArrayKind::Undefined), &name),
                    name: if name.is_empty() {
                        lang.pick("Dispositivo LampArray", "LampArray device")
                            .to_owned()
                    } else {
                        name
                    },
                    vendor: known_rgb_vendor(u32::from(vid)).map(str::to_owned),
                    lamp_count: h.indices.len() as u32,
                    color_capable,
                    color: if color_capable {
                        control.clone()
                    } else {
                        Support::unsupported(lang.pick(
                            "Le luci di questo dispositivo hanno un solo colore.",
                            "This device's lights have a single color.",
                        ))
                    },
                    brightness: control.clone(),
                    effects: control,
                    source: LightingSource::LampArray,
                    id,
                });
            }
        }
        let present: HashSet<String> = out.iter().map(|d| d.id.clone()).collect();
        held.retain(|id, _| present.contains(id));
        drop(held);

        // Known RGB vendors without LampArray: one entry per product.
        let hid = DeviceInformation::FindAllAsyncAqsFilter(&HSTRING::from(HID_AQS))
            .and_then(|op| op.join());
        if let Ok(list) = hid {
            let mut products = HashSet::new();
            for info in &list {
                let Ok(path) = info.Id() else { continue };
                let Some((vid, pid)) = vid_pid(&path.to_string()) else {
                    continue;
                };
                if lamp_products.contains(&(vid, pid)) || !products.insert((vid, pid)) {
                    continue;
                }
                let Some(vendor) = known_rgb_vendor(vid) else {
                    continue;
                };
                let name = info
                    .Name()
                    .map(|n| n.to_string())
                    .ok()
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| vendor.to_owned());
                out.push(vendor_device(
                    format!("{VENDOR_PREFIX}{vid:04x}:{pid:04x}"),
                    name.clone(),
                    vendor,
                    kind_from_name(&name),
                    lang,
                ));
            }
        }
        out
    }

    pub fn layout(&self, id: &str) -> DeviceLayout {
        self.held()
            .ok()
            .and_then(|h| {
                h.get(id).map(|h| DeviceLayout {
                    lamp_count: h.indices.len() as u32,
                    positions: h.positions.clone(),
                })
            })
            .unwrap_or_default()
    }

    pub fn snapshot(&self, id: &str) -> Result<LightingSnapshot, AppError> {
        ensure_apartment();
        let held = self.held()?;
        let opened;
        let h = match held.get(id) {
            Some(h) => h,
            None => {
                opened = Self::open(id)?;
                &opened
            }
        };
        Ok(LightingSnapshot::LampArray {
            autonomous: true,
            enabled: h.array.IsEnabled().ok(),
            brightness: h.array.BrightnessLevel().ok(),
        })
    }

    pub fn take_control(&self, id: &str) -> Result<(), AppError> {
        ensure_apartment();
        let mut held = self.held()?;
        if !held.contains_key(id) {
            let h = Self::open(id)?;
            held.insert(id.to_owned(), h);
        }
        Ok(())
    }

    pub fn write(&self, id: &str, frame: &LightingFrame) -> Result<(), AppError> {
        ensure_apartment();
        let held = self.held()?;
        let h = held.get(id).ok_or_else(|| missing(id))?;
        let color = |c: LightingColor| Color {
            A: 255,
            R: c.r,
            G: c.g,
            B: c.b,
        };
        match frame {
            LightingFrame::Level(l) => {
                let v = (l.clamp(0.0, 1.0) * 255.0).round() as u8;
                h.array
                    .SetColor(color(LightingColor::rgb(v, v, v)))
                    .map_err(platform)
            }
            LightingFrame::Colors(colors) => {
                if colors.windows(2).all(|w| w[0] == w[1]) {
                    let c = colors.first().copied().unwrap_or_default();
                    return h.array.SetColor(color(c)).map_err(platform);
                }
                let n = h.indices.len().min(colors.len());
                let cs: Vec<Color> = colors[..n].iter().map(|c| color(*c)).collect();
                h.array
                    .SetColorsForIndices(&cs, &h.indices[..n])
                    .map_err(platform)
            }
        }
    }

    pub fn restore(&self, id: &str, snapshot: &LightingSnapshot) -> Result<(), AppError> {
        ensure_apartment();
        let mut held = self.held()?;
        if let (
            Some(h),
            LightingSnapshot::LampArray {
                enabled,
                brightness,
                ..
            },
        ) = (held.get(id), snapshot)
        {
            if let Some(e) = enabled
                && h.array.IsEnabled().ok() != Some(*e)
            {
                h.array.SetIsEnabled(*e).map_err(platform)?;
            }
            if let Some(b) = brightness
                && h.array.BrightnessLevel().ok() != Some(*b)
            {
                h.array.SetBrightnessLevel(*b).map_err(platform)?;
            }
        }
        // Dropping the LampArray hands the lamps back to Windows (Dynamic Lighting).
        held.remove(id);
        Ok(())
    }

    pub fn release(&self, id: &str) {
        if let Ok(mut held) = self.held() {
            held.remove(id);
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
    fn vid_pid_parse_from_interface_paths() {
        assert_eq!(
            vid_pid(
                r"\\?\HID#VID_1532&PID_0084&MI_00#7&1a2b#{4d1e55b2-f16f-11cf-88cb-001111000030}"
            ),
            Some((0x1532, 0x0084))
        );
        assert_eq!(vid_pid("nothing"), None);
    }
}
