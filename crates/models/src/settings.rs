use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::device::DeviceShape;
use crate::error::AppError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

/// How much technical detail the UI shows for problems, errors and unavailable values.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ViewMode {
    /// Plain language for non-technical users; technical details on request.
    #[default]
    Simple,
    /// Full technical detail: evidence, confidence, sources, error codes.
    Advanced,
}

/// Interface language chosen in Settings. `System` follows the Windows display language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum LanguagePreference {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "it")]
    Italian,
    #[serde(rename = "en")]
    English,
}

/// Effective language of every user-facing text the backend produces (findings, reports,
/// benchmark labels and notes). Technical error messages (`AppError.message/detail`) are
/// always English; the UI shows a translated title and explanation per `ErrorCode`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Language {
    #[serde(rename = "it")]
    Italian,
    /// Default for every Windows display language other than Italian.
    #[default]
    #[serde(rename = "en")]
    English,
}

impl Language {
    /// Picks the text for this language. Keeps call sites short: `lang.pick("Avviso", "Warning")`.
    pub fn pick<T>(self, italian: T, english: T) -> T {
        match self {
            Self::Italian => italian,
            Self::English => english,
        }
    }

    /// BCP 47 tag, e.g. for number and date formatting.
    pub fn tag(self) -> &'static str {
        self.pick("it-IT", "en-US")
    }
}

impl LanguagePreference {
    /// `os_is_italian`: whether the Windows display language is Italian (any region).
    pub fn resolve(self, os_is_italian: bool) -> Language {
        match self {
            Self::Italian => Language::Italian,
            Self::English => Language::English,
            Self::System if os_is_italian => Language::Italian,
            Self::System => Language::English,
        }
    }
}

pub const SAMPLING_INTERVAL_MIN_MS: u32 = 500;
pub const SAMPLING_INTERVAL_MAX_MS: u32 = 10_000;
pub const PROCESS_TOP_N_MIN: u32 = 1;
pub const PROCESS_TOP_N_MAX: u32 = 50;

/// User preferences. Persisted as JSON; unknown or missing fields fall back to defaults
/// so older databases keep loading after the struct grows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppSettings {
    pub theme: ThemePreference,
    /// Interval for lightweight live metrics. Slow providers use their own longer cadence.
    pub sampling_interval_ms: u32,
    pub sidebar_collapsed: bool,
    pub process_top_n: u32,
    /// Network access (e.g. Windows Update driver search) is off until the user enables it.
    pub allow_online_checks: bool,
    pub view_mode: ViewMode,
    /// "Risolvi" AI assistant. Off by default; needs an API key and per-request consent.
    pub assistant_enabled: bool,
    /// Anthropic model alias, one of `assistant::ASSISTANT_MODELS`.
    pub assistant_model: String,
    /// Interface language. Databases from before this field load as `System`.
    pub language: LanguagePreference,
    /// Device model shown in the Overview, chosen by the user. `None` (the default, and
    /// what databases from before this field load as) follows the detected form factor.
    pub device_shape: Option<DeviceShape>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            sampling_interval_ms: 1_000,
            sidebar_collapsed: false,
            process_top_n: 10,
            allow_online_checks: false,
            view_mode: ViewMode::Simple,
            assistant_enabled: false,
            assistant_model: crate::assistant::DEFAULT_ASSISTANT_MODEL.to_owned(),
            language: LanguagePreference::System,
            device_shape: None,
        }
    }
}

impl AppSettings {
    /// Backend-side validation: the UI is never trusted to send sane values.
    pub fn validate(&self) -> Result<(), AppError> {
        if !(SAMPLING_INTERVAL_MIN_MS..=SAMPLING_INTERVAL_MAX_MS)
            .contains(&self.sampling_interval_ms)
        {
            return Err(AppError::invalid_input(format!(
                "samplingIntervalMs must be between {SAMPLING_INTERVAL_MIN_MS} and {SAMPLING_INTERVAL_MAX_MS}"
            ))
            .with_detail(format!("received {}", self.sampling_interval_ms)));
        }
        if !crate::assistant::ASSISTANT_MODELS.contains(&self.assistant_model.as_str()) {
            return Err(AppError::invalid_input(
                "assistantModel is not one of the supported models",
            )
            .with_detail(format!("received {}", self.assistant_model)));
        }
        if !(PROCESS_TOP_N_MIN..=PROCESS_TOP_N_MAX).contains(&self.process_top_n) {
            return Err(AppError::invalid_input(format!(
                "processTopN must be between {PROCESS_TOP_N_MIN} and {PROCESS_TOP_N_MAX}"
            ))
            .with_detail(format!("received {}", self.process_top_n)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    #[test]
    fn defaults_are_valid_and_offline() {
        let s = AppSettings::default();
        assert!(s.validate().is_ok());
        assert!(!s.allow_online_checks);
        assert_eq!(s.sampling_interval_ms, 1_000);
        assert_eq!(s.process_top_n, 10);
    }

    #[test]
    fn language_preference_resolves() {
        assert_eq!(LanguagePreference::System.resolve(true), Language::Italian);
        assert_eq!(LanguagePreference::System.resolve(false), Language::English);
        assert_eq!(
            LanguagePreference::Italian.resolve(false),
            Language::Italian
        );
        assert_eq!(LanguagePreference::English.resolve(true), Language::English);
        assert_eq!(Language::Italian.pick("a", "b"), "a");
        assert_eq!(Language::English.tag(), "en-US");
    }

    #[test]
    fn older_settings_json_loads_with_system_language() {
        let s: AppSettings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("parse");
        assert_eq!(s.language, LanguagePreference::System);
        let s: AppSettings = serde_json::from_str(r#"{"language":"en"}"#).expect("parse");
        assert_eq!(s.language, LanguagePreference::English);
    }

    #[test]
    fn device_shape_is_optional_and_persists() {
        let s: AppSettings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("parse");
        assert_eq!(s.device_shape, None);
        let s: AppSettings = serde_json::from_str(r#"{"deviceShape":"mini"}"#).expect("parse");
        assert_eq!(s.device_shape, Some(DeviceShape::Mini));
        let json = serde_json::to_value(AppSettings::default()).expect("serialize");
        assert!(json["deviceShape"].is_null());
        assert!(serde_json::from_str::<AppSettings>(r#"{"deviceShape":"phone"}"#).is_err());
    }

    #[test]
    fn rejects_out_of_range_interval() {
        for ms in [
            0,
            SAMPLING_INTERVAL_MIN_MS - 1,
            SAMPLING_INTERVAL_MAX_MS + 1,
        ] {
            let s = AppSettings {
                sampling_interval_ms: ms,
                ..AppSettings::default()
            };
            let err = s.validate().expect_err("must reject");
            assert_eq!(err.code, ErrorCode::InvalidInput);
        }
    }

    #[test]
    fn accepts_interval_bounds() {
        for ms in [SAMPLING_INTERVAL_MIN_MS, SAMPLING_INTERVAL_MAX_MS] {
            let s = AppSettings {
                sampling_interval_ms: ms,
                ..AppSettings::default()
            };
            assert!(s.validate().is_ok());
        }
    }

    #[test]
    fn rejects_out_of_range_top_n() {
        for n in [0, PROCESS_TOP_N_MAX + 1] {
            let s = AppSettings {
                process_top_n: n,
                ..AppSettings::default()
            };
            assert!(s.validate().is_err());
        }
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: AppSettings = serde_json::from_str(r#"{"theme":"dark"}"#).expect("deserialize");
        assert_eq!(s.theme, ThemePreference::Dark);
        assert_eq!(
            s.sampling_interval_ms,
            AppSettings::default().sampling_interval_ms
        );
    }
}
