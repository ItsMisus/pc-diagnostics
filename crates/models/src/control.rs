//! Contracts shared by the four control centers (Performance, Cooling, Lighting, Gaming).
//!
//! Each center owns one submodule. What lives here is the vocabulary they share: whether a
//! control works on this computer, the reference a game profile uses to point at another
//! center's profile, and the power source a setting applies to.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub mod cooling;
pub mod gaming;
pub mod lighting;
pub mod performance;

/// Whether a control can be used on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SupportState {
    /// The app can read and change it.
    Supported,
    /// The app can read it but the system does not let it change it (without admin rights,
    /// a driver or a firmware change, which the app never uses).
    ReadOnly,
    /// Not present or not reachable on this computer.
    Unsupported,
}

/// Support of one control, with the reason in the user's language when it is not
/// `Supported` (what is missing, and where the user can change it instead if anywhere).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Support {
    pub state: SupportState,
    pub reason: Option<String>,
}

impl Support {
    pub fn supported() -> Self {
        Self {
            state: SupportState::Supported,
            reason: None,
        }
    }

    pub fn read_only(reason: impl Into<String>) -> Self {
        Self {
            state: SupportState::ReadOnly,
            reason: Some(reason.into()),
        }
    }

    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            state: SupportState::Unsupported,
            reason: Some(reason.into()),
        }
    }

    pub fn is_supported(&self) -> bool {
        self.state == SupportState::Supported
    }
}

/// The control centers whose profiles a game profile can link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ControlDomainId {
    Performance,
    Cooling,
    Lighting,
}

/// A profile of one control center as other centers see it (Gaming lists these).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileRef {
    pub domain: ControlDomainId,
    pub id: String,
    /// Display name (built-in profiles are named in the user's language).
    pub name: String,
    pub builtin: bool,
    /// False when the profile exists but cannot be applied on this computer.
    pub applicable: bool,
}

/// Power supply a setting applies to (per-source settings). Distinct from
/// `benchmark_run::PowerSource`, which also has `Unknown` for a measured condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PowerSupply {
    /// Mains power (power adapter). Desktops are always on `Ac`.
    Ac,
    Battery,
}

/// Longest name accepted for a user profile, in characters.
pub const PROFILE_NAME_MAX_CHARS: usize = 40;

/// Trims and validates a user-given profile name.
pub fn validate_profile_name(name: &str) -> Result<String, crate::AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(crate::AppError::invalid_input(
            "profile name must not be empty",
        ));
    }
    if name.chars().count() > PROFILE_NAME_MAX_CHARS {
        return Err(crate::AppError::invalid_input(format!(
            "profile name longer than {PROFILE_NAME_MAX_CHARS} characters"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(crate::AppError::invalid_input(
            "profile name contains control characters",
        ));
    }
    Ok(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_names_are_trimmed_and_bounded() {
        assert_eq!(
            validate_profile_name("  Notte ").ok().as_deref(),
            Some("Notte")
        );
        assert!(validate_profile_name("   ").is_err());
        assert!(validate_profile_name(&"x".repeat(PROFILE_NAME_MAX_CHARS + 1)).is_err());
        assert!(validate_profile_name("a\u{0007}").is_err());
    }
}
