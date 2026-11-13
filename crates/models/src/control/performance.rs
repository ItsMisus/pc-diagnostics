//! Performance center contracts. Owned by the performance feature.
//!
//! A performance profile holds one set of settings per power supply (mains and battery).
//! Each setting maps to a documented OS control; what this computer lets the app change is
//! in `PerformanceCapabilities`, what the OS reports right now in `PerformanceOsReport`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{PowerSupply, Support};

/// The power mode of a profile. macOS: Low Power Mode (`Silent`), automatic (`Balanced`),
/// High Power Mode (`HighPerformance`, only on Macs that have it). Windows: the power mode
/// overlay (best power efficiency / balanced / best performance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PerformanceMode {
    Silent,
    Balanced,
    HighPerformance,
}

/// Keeps the computer awake while the profile is active (an assertion held by the app,
/// released on restore, on a profile change and when the app quits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PerformanceKeepAwake {
    /// The OS sleeps as configured.
    #[default]
    Off,
    /// No idle system sleep; the display may still turn off.
    System,
    /// No idle system sleep and no display sleep.
    Display,
}

/// Windows "Processor performance boost mode" (index values 0–6 of the power setting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PerformanceBoostMode {
    Disabled,
    Enabled,
    Aggressive,
    EfficientEnabled,
    EfficientAggressive,
    AggressiveAtGuaranteed,
    EfficientAggressiveAtGuaranteed,
}

impl PerformanceBoostMode {
    pub const ALL: [Self; 7] = [
        Self::Disabled,
        Self::Enabled,
        Self::Aggressive,
        Self::EfficientEnabled,
        Self::EfficientAggressive,
        Self::AggressiveAtGuaranteed,
        Self::EfficientAggressiveAtGuaranteed,
    ];

    /// The value index of the Windows power setting.
    pub fn index(self) -> u32 {
        match self {
            Self::Disabled => 0,
            Self::Enabled => 1,
            Self::Aggressive => 2,
            Self::EfficientEnabled => 3,
            Self::EfficientAggressive => 4,
            Self::AggressiveAtGuaranteed => 5,
            Self::EfficientAggressiveAtGuaranteed => 6,
        }
    }

    pub fn from_index(index: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.index() == index)
    }
}

/// Longest display-off or sleep timeout a profile may set (5 hours), in seconds.
pub const PERFORMANCE_TIMEOUT_MAX_SECONDS: u32 = 5 * 60 * 60;
/// Lowest maximum processor state a profile may set, in percent (below this Windows itself
/// becomes unusable).
pub const PERFORMANCE_PROCESSOR_MAX_FLOOR: u8 = 5;

/// The settings of a profile for one power supply. Advanced values are Windows power
/// settings of the active scheme; `None` leaves the current value as it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PerformanceSupplySettings {
    pub mode: PerformanceMode,
    #[serde(default)]
    pub keep_awake: PerformanceKeepAwake,
    /// "Maximum processor state", percent.
    #[serde(default)]
    pub processor_max_percent: Option<u8>,
    /// "Minimum processor state", percent.
    #[serde(default)]
    pub processor_min_percent: Option<u8>,
    #[serde(default)]
    pub boost_mode: Option<PerformanceBoostMode>,
    /// Turn the display off after this many seconds of inactivity (0 = never).
    #[serde(default)]
    pub display_off_seconds: Option<u32>,
    /// Sleep after this many seconds of inactivity (0 = never).
    #[serde(default)]
    pub sleep_seconds: Option<u32>,
}

impl PerformanceSupplySettings {
    /// Only a mode, every other value left as it is.
    pub fn mode_only(mode: PerformanceMode) -> Self {
        Self {
            mode,
            keep_awake: PerformanceKeepAwake::Off,
            processor_max_percent: None,
            processor_min_percent: None,
            boost_mode: None,
            display_off_seconds: None,
            sleep_seconds: None,
        }
    }

    /// Checks the advanced values against the ranges Windows accepts.
    pub fn validate(&self) -> Result<(), crate::AppError> {
        let bad = |m: &str| Err(crate::AppError::invalid_input(m.to_owned()));
        if let Some(max) = self.processor_max_percent
            && !(PERFORMANCE_PROCESSOR_MAX_FLOOR..=100).contains(&max)
        {
            return bad("maximum processor state out of range");
        }
        if let Some(min) = self.processor_min_percent {
            if min > 100 {
                return bad("minimum processor state out of range");
            }
            if let Some(max) = self.processor_max_percent
                && min > max
            {
                return bad("minimum processor state above the maximum");
            }
        }
        for t in [self.display_off_seconds, self.sleep_seconds]
            .into_iter()
            .flatten()
        {
            if t > PERFORMANCE_TIMEOUT_MAX_SECONDS {
                return bad("timeout longer than 5 hours");
            }
        }
        Ok(())
    }
}

/// A performance profile: built-in (named in the user's language when read) or user-made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PerformanceProfile {
    pub id: String,
    pub name: String,
    pub builtin: bool,
    pub ac: PerformanceSupplySettings,
    pub battery: PerformanceSupplySettings,
}

impl PerformanceProfile {
    pub fn settings_for(&self, supply: PowerSupply) -> &PerformanceSupplySettings {
        match supply {
            PowerSupply::Ac => &self.ac,
            PowerSupply::Battery => &self.battery,
        }
    }
}

/// A user profile to create (`id` null) or replace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PerformanceProfileInput {
    pub id: Option<String>,
    pub name: String,
    pub ac: PerformanceSupplySettings,
    pub battery: PerformanceSupplySettings,
}

/// Every control of the center, for the capability list and for apply issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PerformanceControl {
    Mode,
    KeepAwake,
    ProcessorState,
    BoostMode,
    DisplayTimeout,
    SleepTimeout,
}

impl PerformanceControl {
    pub const ALL: [Self; 6] = [
        Self::Mode,
        Self::KeepAwake,
        Self::ProcessorState,
        Self::BoostMode,
        Self::DisplayTimeout,
        Self::SleepTimeout,
    ];
}

/// What the app can do with each control on this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PerformanceCapabilities {
    pub mode: Support,
    pub keep_awake: Support,
    pub processor_state: Support,
    pub boost_mode: Support,
    pub display_timeout: Support,
    pub sleep_timeout: Support,
}

impl PerformanceCapabilities {
    pub fn get(&self, control: PerformanceControl) -> &Support {
        match control {
            PerformanceControl::Mode => &self.mode,
            PerformanceControl::KeepAwake => &self.keep_awake,
            PerformanceControl::ProcessorState => &self.processor_state,
            PerformanceControl::BoostMode => &self.boost_mode,
            PerformanceControl::DisplayTimeout => &self.display_timeout,
            PerformanceControl::SleepTimeout => &self.sleep_timeout,
        }
    }
}

/// What the OS reports for one power supply. `None`: not reported here (or not readable).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PerformanceOsSupply {
    /// macOS: Low / High Power Mode for this supply. Windows: the power mode overlay (only
    /// for the current supply: Windows reports the effective one).
    #[serde(default)]
    pub mode: Option<PerformanceMode>,
    #[serde(default)]
    pub processor_max_percent: Option<u8>,
    #[serde(default)]
    pub processor_min_percent: Option<u8>,
    #[serde(default)]
    pub boost_mode: Option<PerformanceBoostMode>,
    #[serde(default)]
    pub display_off_seconds: Option<u32>,
    #[serde(default)]
    pub sleep_seconds: Option<u32>,
}

/// The system's own power settings right now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PerformanceOsReport {
    pub ac: PerformanceOsSupply,
    /// Null without a battery.
    pub battery: Option<PerformanceOsSupply>,
    /// Windows: friendly name of the active power scheme.
    pub scheme_name: Option<String>,
    /// The keep-awake assertion the app holds now.
    pub keep_awake: PerformanceKeepAwake,
}

/// Something the last apply could not change, in the user's language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PerformanceIssue {
    pub control: PerformanceControl,
    pub message: String,
}

/// The game override set by the Gaming center, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PerformanceGameOverride {
    pub profile_id: String,
    pub profile_name: String,
}

/// Everything the Performance page shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PerformanceState {
    pub capabilities: PerformanceCapabilities,
    pub os: PerformanceOsReport,
    /// The supply powering the computer now (always `ac` without a battery).
    pub supply: PowerSupply,
    pub has_battery: bool,
    /// Built-in profiles first, then the user's.
    pub profiles: Vec<PerformanceProfile>,
    /// The user's own selection (null: none, the system's own settings).
    pub selected_profile_id: Option<String>,
    pub game_override: Option<PerformanceGameOverride>,
    /// True when the original system state is stored and "Ripristina" can bring it back.
    pub can_restore: bool,
    /// What the last apply could not change.
    pub issues: Vec<PerformanceIssue>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boost_mode_indices_round_trip() {
        for m in PerformanceBoostMode::ALL {
            assert_eq!(PerformanceBoostMode::from_index(m.index()), Some(m));
        }
        assert_eq!(PerformanceBoostMode::from_index(7), None);
    }

    #[test]
    fn supply_settings_are_validated() {
        let mut s = PerformanceSupplySettings::mode_only(PerformanceMode::Balanced);
        assert!(s.validate().is_ok());
        s.processor_max_percent = Some(4);
        assert!(s.validate().is_err());
        s.processor_max_percent = Some(80);
        s.processor_min_percent = Some(90);
        assert!(s.validate().is_err());
        s.processor_min_percent = Some(5);
        s.sleep_seconds = Some(PERFORMANCE_TIMEOUT_MAX_SECONDS + 1);
        assert!(s.validate().is_err());
        s.sleep_seconds = Some(0);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn missing_optional_fields_deserialize() {
        let s: PerformanceSupplySettings =
            serde_json::from_str(r#"{"mode":"silent"}"#).expect("parse");
        assert_eq!(
            s,
            PerformanceSupplySettings::mode_only(PerformanceMode::Silent)
        );
    }
}
