//! OS access for the lighting center. Owned by the lighting feature.
//!
//! - Standard RGB devices: HID LampArray (usage page 0x59). macOS: IOHIDManager and feature
//!   reports (`lighting/macos_hid.rs`, report encoding in `lamparray.rs`). Windows:
//!   `Windows.Devices.Lights.LampArray` (`lighting/windows.rs`), which only the foreground app
//!   or the app chosen in Settings > Personalization > Dynamic Lighting may drive.
//! - Mac keyboard backlight: CoreBrightness `KeyboardBrightnessClient`, loaded at run time
//!   (`lighting/macos_backlight.rs`); absent class or backlight = no device.
//! - Devices of known RGB vendors without LampArray are listed as recognised, never driven
//!   (`vendors.rs`).
//!
//! No admin rights, no driver, nothing persistent on the device: LampArray colors last until
//! the device is handed back (autonomous mode), the backlight level is the user's own
//! setting that the keyboard keys also change.

use models::AppError;
use models::control::lighting::{LightingColor, LightingDevice, LightingSnapshot};
use models::settings::Language;

pub mod lamparray;
pub mod vendors;

#[cfg(target_os = "macos")]
mod macos_backlight;
#[cfg(target_os = "macos")]
mod macos_hid;
#[cfg(windows)]
mod windows;

/// Where the lamps of a device sit, for effects that move across them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeviceLayout {
    pub lamp_count: u32,
    /// Normalized x of each lamp, 0 (left) to 1 (right); empty = spread evenly.
    pub positions: Vec<f64>,
}

/// One update of a device.
#[derive(Debug, Clone, PartialEq)]
pub enum LightingFrame {
    /// One color per lamp, in lamp order (already dimmed by the brightness).
    Colors(Vec<LightingColor>),
    /// Level 0–1 of a single-color light.
    Level(f64),
}

/// Normalizes raw lamp x coordinates to 0..=1 over their own extent (all equal: 0.5).
pub fn normalize_positions(xs: &[f64]) -> Vec<f64> {
    let min = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let max = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !min.is_finite() || !max.is_finite() || max - min <= f64::EPSILON {
        return vec![0.5; xs.len()];
    }
    xs.iter().map(|x| (x - min) / (max - min)).collect()
}

/// The lighting devices of this computer.
pub struct SystemLighting {
    #[cfg(target_os = "macos")]
    backlight: macos_backlight::Backlight,
    #[cfg(target_os = "macos")]
    hid: macos_hid::HidLighting,
    #[cfg(windows)]
    win: windows::WinLighting,
}

impl Default for SystemLighting {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemLighting {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            backlight: macos_backlight::Backlight::new(),
            #[cfg(target_os = "macos")]
            hid: macos_hid::HidLighting::new(),
            #[cfg(windows)]
            win: windows::WinLighting::new(),
        }
    }

    /// Enumerates the devices (controllable and recognised ones). Never fails: a missing
    /// API leaves its devices out.
    pub fn devices(&self, lang: Language) -> Vec<LightingDevice> {
        #[allow(unused_mut)]
        let mut out: Vec<LightingDevice> = Vec::new();
        #[cfg(target_os = "macos")]
        {
            out.extend(self.backlight.devices(lang));
            out.extend(self.hid.devices(lang));
        }
        #[cfg(windows)]
        out.extend(self.win.devices(lang));
        let _ = lang;
        out
    }

    pub fn layout(&self, id: &str) -> DeviceLayout {
        #[cfg(target_os = "macos")]
        if macos_hid::owns(id) {
            return self.hid.layout(id);
        }
        #[cfg(windows)]
        if windows::owns(id) {
            return self.win.layout(id);
        }
        let _ = id;
        DeviceLayout {
            lamp_count: 1,
            positions: Vec::new(),
        }
    }

    /// Reads what the device shows now, to put it back later.
    pub fn snapshot(&self, id: &str) -> Result<LightingSnapshot, AppError> {
        #[cfg(target_os = "macos")]
        {
            if macos_backlight::owns(id) {
                return self.backlight.snapshot(id);
            }
            if macos_hid::owns(id) {
                return Ok(self.hid.snapshot(id));
            }
        }
        #[cfg(windows)]
        if windows::owns(id) {
            return self.win.snapshot(id);
        }
        Err(unknown(id))
    }

    /// Takes the device over (LampArray: autonomous mode off).
    pub fn take_control(&self, id: &str) -> Result<(), AppError> {
        #[cfg(target_os = "macos")]
        {
            if macos_backlight::owns(id) {
                return Ok(());
            }
            if macos_hid::owns(id) {
                return self.hid.take_control(id);
            }
        }
        #[cfg(windows)]
        if windows::owns(id) {
            return self.win.take_control(id);
        }
        Err(unknown(id))
    }

    /// Shows one frame. `persist` marks a frame that should stick (a steady effect, a
    /// restore), as opposed to an animation step.
    pub fn write(&self, id: &str, frame: &LightingFrame, persist: bool) -> Result<(), AppError> {
        #[cfg(target_os = "macos")]
        {
            if macos_backlight::owns(id) {
                return self.backlight.write(id, frame, persist);
            }
            if macos_hid::owns(id) {
                return self.hid.write(id, frame);
            }
        }
        #[cfg(windows)]
        if windows::owns(id) {
            return self.win.write(id, frame);
        }
        let _ = (frame, persist);
        Err(unknown(id))
    }

    /// Puts the device back as `snapshot` describes and hands it back.
    pub fn restore(&self, id: &str, snapshot: &LightingSnapshot) -> Result<(), AppError> {
        #[cfg(target_os = "macos")]
        {
            if macos_backlight::owns(id) {
                return self.backlight.restore(id, snapshot);
            }
            if macos_hid::owns(id) {
                return self.hid.restore(id, snapshot);
            }
        }
        #[cfg(windows)]
        if windows::owns(id) {
            return self.win.restore(id, snapshot);
        }
        let _ = snapshot;
        Err(unknown(id))
    }

    /// Hands the device back without restoring colors (LampArray: autonomous mode on).
    pub fn release(&self, id: &str) {
        #[cfg(target_os = "macos")]
        if macos_hid::owns(id) {
            self.hid.release(id);
        }
        #[cfg(windows)]
        if windows::owns(id) {
            self.win.release(id);
        }
        let _ = id;
    }

    /// A platform-wide limit worth one line on the page.
    pub fn platform_note(&self, lang: Language) -> Option<String> {
        #[cfg(windows)]
        return Some(windows::dynamic_lighting_note(lang).to_owned());
        #[cfg(not(windows))]
        {
            let _ = lang;
            None
        }
    }
}

fn unknown(id: &str) -> AppError {
    AppError::new(
        models::ErrorCode::NotFound,
        format!("lighting device {id} not found"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_normalize_over_their_extent() {
        assert_eq!(
            normalize_positions(&[10.0, 20.0, 30.0]),
            vec![0.0, 0.5, 1.0]
        );
        assert_eq!(normalize_positions(&[5.0, 5.0]), vec![0.5, 0.5]);
        assert!(normalize_positions(&[]).is_empty());
    }

    /// Lists the devices of the computer running the tests (read only).
    #[test]
    fn host_devices_enumerate() {
        let sys = SystemLighting::new();
        for d in sys.devices(Language::English) {
            eprintln!(
                "{} {:?} {:?} lamps={} color={:?} brightness={:?}",
                d.name, d.kind, d.source, d.lamp_count, d.color.state, d.brightness.state
            );
            assert!(!d.id.is_empty());
        }
    }
}
