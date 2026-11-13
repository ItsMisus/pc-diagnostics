//! Cooling center contracts. Owned by the cooling feature.
//!
//! What the computer cools itself with (fans, passive), the fans it reports, the cooling
//! profiles (built-in and user-made, with fan curves and the Windows "System cooling policy"
//! per power supply) and what of it the app can really change here.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{PowerSupply, Support};
use crate::AppError;

/// How this computer is cooled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CoolingSystemKind {
    /// One or more fans were found.
    Fans,
    /// No fan: heat leaves through the chassis (e.g. MacBook Air).
    Passive,
    /// The system does not say (e.g. a Windows PC whose firmware reports no fan to WMI).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingSystem {
    pub kind: CoolingSystemKind,
    /// One sentence in the user's language: what was found and where it comes from.
    pub description: String,
}

/// Who sets a fan's speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CoolingFanMode {
    /// The system controller (SMC, firmware) follows its own curve.
    Auto,
    /// A fixed target was forced (by another tool or the firmware).
    Forced,
}

/// One fan as the system reports it. Speeds the system does not report are `None`, with
/// `rpm_reason` saying why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingFan {
    pub id: String,
    pub label: String,
    pub current_rpm: Option<f64>,
    pub min_rpm: Option<f64>,
    pub max_rpm: Option<f64>,
    pub target_rpm: Option<f64>,
    /// Why some of the speeds above are missing (user's language).
    pub rpm_reason: Option<String>,
    pub mode: Option<CoolingFanMode>,
    /// Whether the app can set this fan's speed.
    pub speed_control: Support,
}

/// The kind of a cooling profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CoolingMode {
    /// The system's own behaviour (the values found before the app changed anything).
    Automatic,
    Silent,
    Balanced,
    /// A user profile with its own curves and policy.
    Custom,
}

/// One point of a fan curve: at `temperature_c` the fan runs at `duty_percent`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingCurvePoint {
    pub temperature_c: f64,
    pub duty_percent: f64,
}

pub const CURVE_MIN_POINTS: usize = 2;
pub const CURVE_MAX_POINTS: usize = 8;
pub const CURVE_TEMP_MIN_C: f64 = 20.0;
pub const CURVE_TEMP_MAX_C: f64 = 100.0;

/// A fan curve: 2–8 points, temperatures in 20–100 °C strictly increasing, duty in 0–100 %
/// never decreasing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingCurve {
    pub points: Vec<CoolingCurvePoint>,
}

impl CoolingCurve {
    pub fn new(points: &[(f64, f64)]) -> Self {
        Self {
            points: points
                .iter()
                .map(|&(temperature_c, duty_percent)| CoolingCurvePoint {
                    temperature_c,
                    duty_percent,
                })
                .collect(),
        }
    }

    pub fn validate(&self) -> Result<(), AppError> {
        let n = self.points.len();
        if !(CURVE_MIN_POINTS..=CURVE_MAX_POINTS).contains(&n) {
            return Err(AppError::invalid_input(format!(
                "a fan curve needs {CURVE_MIN_POINTS} to {CURVE_MAX_POINTS} points, got {n}"
            )));
        }
        for p in &self.points {
            if !p.temperature_c.is_finite()
                || !(CURVE_TEMP_MIN_C..=CURVE_TEMP_MAX_C).contains(&p.temperature_c)
            {
                return Err(AppError::invalid_input(format!(
                    "curve temperature {} °C outside {CURVE_TEMP_MIN_C}–{CURVE_TEMP_MAX_C} °C",
                    p.temperature_c
                )));
            }
            if !p.duty_percent.is_finite() || !(0.0..=100.0).contains(&p.duty_percent) {
                return Err(AppError::invalid_input(format!(
                    "curve duty {} % outside 0–100 %",
                    p.duty_percent
                )));
            }
        }
        for w in self.points.windows(2) {
            if w[1].temperature_c <= w[0].temperature_c {
                return Err(AppError::invalid_input(
                    "curve temperatures must be strictly increasing",
                ));
            }
            if w[1].duty_percent < w[0].duty_percent {
                return Err(AppError::invalid_input(
                    "curve duty must not decrease as the temperature rises",
                ));
            }
        }
        Ok(())
    }

    /// Duty at `temperature_c`: linear between points, flat beyond the ends.
    pub fn duty_at(&self, temperature_c: f64) -> Option<f64> {
        let first = self.points.first()?;
        let last = self.points.last()?;
        if temperature_c <= first.temperature_c {
            return Some(first.duty_percent);
        }
        if temperature_c >= last.temperature_c {
            return Some(last.duty_percent);
        }
        self.points.windows(2).find_map(|w| {
            let (a, b) = (w[0], w[1]);
            (temperature_c >= a.temperature_c && temperature_c <= b.temperature_c).then(|| {
                let f = (temperature_c - a.temperature_c) / (b.temperature_c - a.temperature_c);
                a.duty_percent + f * (b.duty_percent - a.duty_percent)
            })
        })
    }

    /// The quiet preset: fans slow until the chip is warm.
    pub fn silent() -> Self {
        Self::new(&[(30.0, 0.0), (55.0, 20.0), (75.0, 45.0), (90.0, 80.0)])
    }

    pub fn balanced() -> Self {
        Self::new(&[(30.0, 20.0), (50.0, 35.0), (70.0, 60.0), (85.0, 100.0)])
    }

    pub fn aggressive() -> Self {
        Self::new(&[(30.0, 35.0), (45.0, 50.0), (60.0, 75.0), (75.0, 100.0)])
    }
}

/// A curve for every fan (`fan_id` null) or for one fan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingFanCurve {
    pub fan_id: Option<String>,
    pub curve: CoolingCurve,
}

/// The Windows "System cooling policy" power setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CoolingPolicy {
    /// Slow the processor before speeding up the fans (value 0).
    Passive,
    /// Speed up the fans before slowing the processor (value 1).
    Active,
}

impl CoolingPolicy {
    pub fn from_index(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Passive),
            1 => Some(Self::Active),
            _ => None,
        }
    }

    pub fn index(self) -> u32 {
        match self {
            Self::Passive => 0,
            Self::Active => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingPolicySetting {
    pub supply: PowerSupply,
    pub policy: CoolingPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingProfile {
    pub id: String,
    /// Built-in profiles are named in the user's language.
    pub name: String,
    pub builtin: bool,
    pub mode: CoolingMode,
    /// Fan curves (an entry with `fan_id` null applies to every fan without its own).
    pub curves: Vec<CoolingFanCurve>,
    /// Cooling policy per supply. A supply that is not listed keeps the value the system
    /// had before the app changed anything.
    pub cooling_policy: Vec<CoolingPolicySetting>,
}

impl CoolingProfile {
    pub fn validate(&self) -> Result<(), AppError> {
        for c in &self.curves {
            c.curve.validate()?;
        }
        let mut seen = Vec::new();
        for c in &self.curves {
            if seen.contains(&&c.fan_id) {
                return Err(AppError::invalid_input("two curves for the same fan"));
            }
            seen.push(&c.fan_id);
        }
        for (i, a) in self.cooling_policy.iter().enumerate() {
            if self.cooling_policy[..i]
                .iter()
                .any(|b| b.supply == a.supply)
            {
                return Err(AppError::invalid_input(
                    "cooling policy listed twice for one supply",
                ));
            }
        }
        Ok(())
    }
}

/// A profile with what it can do on this computer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingProfileEntry {
    pub profile: CoolingProfile,
    /// False when nothing in it can be applied here.
    pub applicable: bool,
    /// What applying it does here, or why it cannot (user's language).
    pub note: Option<String>,
}

/// What a user profile is made of (new: `id` null).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingProfileDraft {
    pub id: Option<String>,
    pub name: String,
    pub curves: Vec<CoolingFanCurve>,
    pub cooling_policy: Vec<CoolingPolicySetting>,
}

/// Support of each kind of control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingCapabilities {
    /// Setting a fan's speed.
    pub fan_speed: Support,
    /// Applying a fan curve (needs fan speed control).
    pub curves: Support,
    /// The Windows "System cooling policy".
    pub cooling_policy: Support,
}

/// Everything the cooling page shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoolingState {
    pub system: CoolingSystem,
    pub fans: Vec<CoolingFan>,
    pub capabilities: CoolingCapabilities,
    /// The cooling policy now in effect, per supply (empty when unreadable).
    pub current_policy: Vec<CoolingPolicySetting>,
    pub profiles: Vec<CoolingProfileEntry>,
    /// The user's own selection (null: none applied by the app).
    pub selected: Option<String>,
    /// The profile set by Gaming while a game runs.
    pub game_override: Option<String>,
    /// The profile in effect: the override, the selection, or "auto" when the system is
    /// as the app found it.
    pub active: Option<String>,
    /// The original state was stored and can be put back.
    pub can_restore: bool,
    /// CPU temperature now, for the curve editor's marker (null when not readable).
    pub cpu_temperature_c: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_valid_and_curves_validate() {
        for c in [
            CoolingCurve::silent(),
            CoolingCurve::balanced(),
            CoolingCurve::aggressive(),
        ] {
            assert!(c.validate().is_ok());
        }
        assert!(CoolingCurve::new(&[(30.0, 10.0)]).validate().is_err());
        assert!(
            CoolingCurve::new(&[(30.0, 10.0), (30.0, 20.0)])
                .validate()
                .is_err()
        );
        assert!(
            CoolingCurve::new(&[(30.0, 50.0), (40.0, 20.0)])
                .validate()
                .is_err()
        );
        assert!(
            CoolingCurve::new(&[(10.0, 0.0), (40.0, 20.0)])
                .validate()
                .is_err()
        );
        assert!(
            CoolingCurve::new(&[(30.0, 0.0), (40.0, 120.0)])
                .validate()
                .is_err()
        );
        assert!(
            CoolingCurve::new(&[(30.0, f64::NAN), (40.0, 20.0)])
                .validate()
                .is_err()
        );
        let nine: Vec<(f64, f64)> = (0..9).map(|i| (20.0 + f64::from(i) * 5.0, 50.0)).collect();
        assert!(CoolingCurve::new(&nine).validate().is_err());
        assert!(
            CoolingCurve::new(&[(30.0, 20.0), (40.0, 20.0)])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn duty_interpolates_and_clamps() {
        let c = CoolingCurve::new(&[(40.0, 20.0), (60.0, 60.0)]);
        assert_eq!(c.duty_at(30.0), Some(20.0));
        assert_eq!(c.duty_at(50.0), Some(40.0));
        assert_eq!(c.duty_at(90.0), Some(60.0));
        assert_eq!(CoolingCurve { points: vec![] }.duty_at(50.0), None);
    }

    #[test]
    fn policy_indices_round_trip() {
        for p in [CoolingPolicy::Passive, CoolingPolicy::Active] {
            assert_eq!(CoolingPolicy::from_index(p.index()), Some(p));
        }
        assert_eq!(CoolingPolicy::from_index(7), None);
    }

    #[test]
    fn profiles_reject_duplicates() {
        let mut p = CoolingProfile {
            id: "x".into(),
            name: "X".into(),
            builtin: false,
            mode: CoolingMode::Custom,
            curves: vec![CoolingFanCurve {
                fan_id: None,
                curve: CoolingCurve::balanced(),
            }],
            cooling_policy: vec![CoolingPolicySetting {
                supply: PowerSupply::Ac,
                policy: CoolingPolicy::Active,
            }],
        };
        assert!(p.validate().is_ok());
        p.cooling_policy.push(p.cooling_policy[0]);
        assert!(p.validate().is_err());
        p.cooling_policy.pop();
        p.curves.push(p.curves[0].clone());
        assert!(p.validate().is_err());
    }
}
