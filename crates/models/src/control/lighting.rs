//! Lighting center contracts. Owned by the lighting feature.
//!
//! Devices the app found (standard HID LampArray devices, the Mac keyboard backlight, and
//! devices of known RGB vendors that only speak a proprietary protocol), the effects it can
//! render, the user's profiles and the state the page shows. The effect math itself lives in
//! `control::lighting::effect` (and its TypeScript port in `src/features/lighting/effect.ts`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Support;

/// An sRGB color, 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LightingColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl LightingColor {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// What kind of device carries the lights (picks the icon and the 3D model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LightingDeviceKind {
    Keyboard,
    Mouse,
    Headset,
    Strip,
    Fan,
    Case,
    /// A white (single color) keyboard backlight, such as a MacBook's.
    KeyboardBacklight,
    Other,
}

/// How the app reaches the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LightingSource {
    /// HID LampArray (usage page 0x59): Windows Dynamic Lighting, IOHIDManager on macOS.
    LampArray,
    /// The built-in keyboard backlight of a Mac (CoreBrightness).
    MacKeyboardBacklight,
    /// A device of a known RGB vendor without LampArray: recognised, never controlled.
    VendorProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LightingDevice {
    /// Stable for the device while it stays connected (and usually across restarts).
    pub id: String,
    pub name: String,
    pub kind: LightingDeviceKind,
    /// Manufacturer as reported (or known from the USB vendor id).
    pub vendor: Option<String>,
    pub lamp_count: u32,
    /// True for RGB lamps; false for single-color (white) lights.
    pub color_capable: bool,
    pub color: Support,
    pub brightness: Support,
    pub effects: Support,
    pub source: LightingSource,
}

impl LightingDevice {
    /// The app can change this device's light at all.
    pub fn controllable(&self) -> bool {
        self.brightness.is_supported() || self.color.is_supported()
    }
}

/// Direction of a moving effect, in the lamps' own layout (left = smaller x).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LightingDirection {
    #[default]
    LeftToRight,
    RightToLeft,
}

/// Slowest and fastest effect speed (`speed` of the dynamic effects).
pub const LIGHTING_SPEED_MIN: u8 = 1;
pub const LIGHTING_SPEED_MAX: u8 = 10;
/// Breathing period bounds, in milliseconds.
pub const LIGHTING_PERIOD_MIN_MS: u32 = 1_000;
pub const LIGHTING_PERIOD_MAX_MS: u32 = 12_000;
/// Most colors a multi-color effect accepts.
pub const LIGHTING_MAX_COLORS: usize = 8;

/// A lighting effect. `speed` is 1 (slow) to 10 (fast); one cycle lasts `12 s / speed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum LightingEffect {
    Off,
    Static {
        color: LightingColor,
    },
    Breathing {
        color: LightingColor,
        /// One full breath, in milliseconds.
        period: u32,
    },
    Rainbow {
        speed: u8,
    },
    Wave {
        colors: Vec<LightingColor>,
        speed: u8,
        direction: LightingDirection,
    },
    Gradient {
        colors: Vec<LightingColor>,
    },
    ColorCycle {
        colors: Vec<LightingColor>,
        speed: u8,
    },
}

impl LightingEffect {
    /// The light changes over time (the engine renders frames).
    pub fn is_dynamic(&self) -> bool {
        matches!(
            self,
            Self::Breathing { .. }
                | Self::Rainbow { .. }
                | Self::Wave { .. }
                | Self::ColorCycle { .. }
        )
    }

    /// A single-color light can show it (brightness envelope only: off, steady, breathing).
    pub fn monochrome_compatible(&self) -> bool {
        matches!(
            self,
            Self::Off | Self::Static { .. } | Self::Breathing { .. }
        )
    }

    /// Checks the parameters (speed, period, color count).
    pub fn validate(&self) -> Result<(), crate::AppError> {
        let speed_ok = |s: u8| (LIGHTING_SPEED_MIN..=LIGHTING_SPEED_MAX).contains(&s);
        let colors_ok = |c: &[LightingColor]| !c.is_empty() && c.len() <= LIGHTING_MAX_COLORS;
        let ok = match self {
            Self::Off | Self::Static { .. } => true,
            Self::Breathing { period, .. } => {
                (LIGHTING_PERIOD_MIN_MS..=LIGHTING_PERIOD_MAX_MS).contains(period)
            }
            Self::Rainbow { speed } => speed_ok(*speed),
            Self::Wave { colors, speed, .. } | Self::ColorCycle { colors, speed } => {
                speed_ok(*speed) && colors_ok(colors)
            }
            Self::Gradient { colors } => colors_ok(colors),
        };
        if ok {
            Ok(())
        } else {
            Err(crate::AppError::invalid_input(
                "lighting effect parameters out of range",
            ))
        }
    }
}

/// Which devices a profile drives.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum LightingTargets {
    /// Every controllable device, including ones connected later.
    #[default]
    All,
    /// Only these device ids.
    Devices { ids: Vec<String> },
}

impl LightingTargets {
    pub fn includes(&self, id: &str) -> bool {
        match self {
            Self::All => true,
            Self::Devices { ids } => ids.iter().any(|x| x == id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LightingProfile {
    pub id: String,
    /// Built-in profiles are named in the user's language.
    pub name: String,
    pub builtin: bool,
    pub effect: LightingEffect,
    /// 0–100.
    pub brightness: u8,
    pub targets: LightingTargets,
    /// One timeline and one layout across devices (a wave flows from one to the next).
    pub sync: bool,
}

/// What a request to apply (or preview) carries: the profile body without identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LightingSetup {
    pub effect: LightingEffect,
    pub brightness: u8,
    pub targets: LightingTargets,
    pub sync: bool,
}

/// What is on the devices now (set by the app in this session).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LightingApplied {
    pub setup: LightingSetup,
    /// The profile it came from, if any.
    pub profile_id: Option<String>,
    /// Ids of the devices that took it.
    pub device_ids: Vec<String>,
}

/// Everything the lighting page shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LightingState {
    /// Controllable devices first, then recognised but unsupported ones.
    pub devices: Vec<LightingDevice>,
    pub profiles: Vec<LightingProfile>,
    /// The user's selected profile (kept across restarts; never applied at startup).
    pub selected: Option<String>,
    /// Profile id applied by Gaming while a game runs.
    pub game_override: Option<String>,
    /// What the app set, if anything, in this session.
    pub applied: Option<LightingApplied>,
    /// An original state is stored and "Ripristina" can put it back.
    pub can_restore: bool,
    /// The background renderer is running (a dynamic effect on a real device).
    pub engine_running: bool,
    /// One line about this platform's limits (e.g. Windows Dynamic Lighting), if any.
    pub platform_note: Option<String>,
}

/// What a device looked like before the app first changed it (stored in
/// `lighting.original`). Internal, not part of the IPC contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LightingSnapshot {
    /// Mac keyboard backlight: brightness 0–1 and the auto-brightness flag as read.
    Backlight { level: f32, auto: bool },
    /// LampArray: whether the device ran its own (autonomous) effect; Windows also keeps
    /// the enabled flag and brightness level.
    LampArray {
        autonomous: bool,
        enabled: Option<bool>,
        brightness: Option<f64>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_serialize_with_a_kind_tag() {
        let e = LightingEffect::Wave {
            colors: vec![LightingColor::rgb(255, 0, 0)],
            speed: 3,
            direction: LightingDirection::RightToLeft,
        };
        let json = serde_json::to_value(&e).expect("serialize");
        assert_eq!(json["kind"], "wave");
        assert_eq!(json["direction"], "rightToLeft");
        assert_eq!(json["colors"][0]["r"], 255);
        let back: LightingEffect = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, e);
        let t = serde_json::to_value(LightingTargets::Devices {
            ids: vec!["a".into()],
        })
        .expect("serialize");
        assert_eq!(t["kind"], "devices");
    }

    #[test]
    fn effect_parameters_are_bounded() {
        assert!(LightingEffect::Rainbow { speed: 0 }.validate().is_err());
        assert!(LightingEffect::Rainbow { speed: 10 }.validate().is_ok());
        assert!(
            LightingEffect::Breathing {
                color: LightingColor::WHITE,
                period: 500
            }
            .validate()
            .is_err()
        );
        assert!(
            LightingEffect::Gradient { colors: vec![] }
                .validate()
                .is_err()
        );
        assert!(
            LightingEffect::ColorCycle {
                colors: vec![LightingColor::WHITE; 9],
                speed: 2
            }
            .validate()
            .is_err()
        );
        assert!(LightingEffect::Off.validate().is_ok());
        assert!(LightingEffect::Rainbow { speed: 1 }.is_dynamic());
        assert!(!LightingEffect::Rainbow { speed: 1 }.monochrome_compatible());
    }

    #[test]
    fn snapshot_round_trips() {
        let s = LightingSnapshot::Backlight {
            level: 0.5,
            auto: true,
        };
        let json = serde_json::to_string(&s).expect("serialize");
        assert!(json.contains("\"kind\":\"backlight\""));
        let back: LightingSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, s);
    }
}
