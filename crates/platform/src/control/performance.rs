//! OS access for the performance center. Owned by the performance feature.
//!
//! - macOS: Low Power Mode, High Power Mode and the sleep timers are read per supply
//!   (`IOPMCopyPMPreferences`, an exported I/O Kit call without entitlement; falls back to
//!   `NSProcessInfo.isLowPowerModeEnabled`). Changing them needs root (`pmset`), so they are
//!   read-only here. Keep-awake uses `IOPMAssertionCreateWithName`, no privileges.
//! - Windows: the power mode overlay (`PowerGetEffectiveOverlayScheme` /
//!   `PowerSetActiveOverlayScheme`, resolved at run time from powrprof.dll), per-supply values
//!   of the active scheme (`PowerRead/WriteAC/DCValueIndex` + `PowerSetActiveScheme`) and
//!   keep-awake via `SetThreadExecutionState` on a dedicated thread.
//! - Elsewhere: everything unsupported.
//!
//! This module returns facts without language; the controller words the reasons.

use models::AppError;
use models::control::PowerSupply;
use models::control::performance::{PerformanceControl, PerformanceKeepAwake, PerformanceOsSupply};

#[cfg(target_os = "macos")]
#[path = "performance/macos.rs"]
mod imp;

#[cfg(windows)]
#[path = "performance/windows.rs"]
mod imp;

#[cfg(not(any(target_os = "macos", windows)))]
#[path = "performance/other.rs"]
mod imp;

/// How the OS lets the app use one control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Access {
    /// The app can read and change it.
    Writable,
    /// Readable, but changing it needs administrator rights (never requested).
    NeedsAdmin,
    /// A policy or the system refused a change (access denied).
    Denied,
    /// This OS has no such control.
    NotOnThisOs,
    /// Not present on this computer or OS version (model without High Power Mode, an API
    /// missing from an older Windows, a scheme without the setting).
    NotAvailable,
}

/// Access to every control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceAccess {
    pub mode: Access,
    pub keep_awake: Access,
    pub processor_state: Access,
    pub boost_mode: Access,
    pub display_timeout: Access,
    pub sleep_timeout: Access,
}

impl PerformanceAccess {
    pub fn get(&self, control: PerformanceControl) -> &Access {
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

/// The power state the OS reports now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsPowerState {
    pub supply: PowerSupply,
    pub has_battery: bool,
    pub ac: PerformanceOsSupply,
    /// `None` without a battery.
    pub battery: Option<PerformanceOsSupply>,
    /// Windows: the active scheme (GUID string and friendly name).
    pub scheme_id: Option<String>,
    pub scheme_name: Option<String>,
    pub access: PerformanceAccess,
}

/// One value the OS did not accept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteFailure {
    pub control: PerformanceControl,
    /// Access denied (policy, permissions): the control is then reported read-only.
    pub denied: bool,
    pub detail: String,
}

/// Reads everything (a few system calls; call off the UI thread).
pub fn read_state() -> OsPowerState {
    imp::read_state()
}

/// The supply powering the computer now and whether it has a battery (cheap, for polling).
pub fn current_supply() -> (PowerSupply, bool) {
    imp::current_supply()
}

/// Writes the given values (`None` fields are left alone). Advanced values go to both
/// supplies of the scheme `scheme_id` (the active one when `None`), the mode only to the
/// supply `current` (the OS applies it to the supply in use). Returns what failed.
pub fn write_settings(
    scheme_id: Option<&str>,
    current: PowerSupply,
    ac: &PerformanceOsSupply,
    battery: Option<&PerformanceOsSupply>,
) -> Vec<WriteFailure> {
    imp::write_settings(scheme_id, current, ac, battery)
}

/// Opens the OS power settings (macOS: System Settings > Battery / Energy; Windows:
/// Settings > System > Power). Fixed targets only.
pub fn open_power_settings() -> Result<(), AppError> {
    imp::open_power_settings()
}

/// A keep-awake request held by the app. Dropping it releases it.
pub struct KeepAwake {
    inner: imp::KeepAwakeImpl,
    current: PerformanceKeepAwake,
}

impl KeepAwake {
    pub fn new() -> Self {
        Self {
            inner: imp::KeepAwakeImpl::default(),
            current: PerformanceKeepAwake::Off,
        }
    }

    pub fn current(&self) -> PerformanceKeepAwake {
        self.current
    }

    /// Switches to `mode` (releasing the previous request first). On failure nothing is
    /// held.
    pub fn set(&mut self, mode: PerformanceKeepAwake) -> Result<(), AppError> {
        if mode == self.current {
            return Ok(());
        }
        let result = self.inner.set(mode);
        self.current = if result.is_ok() {
            mode
        } else {
            PerformanceKeepAwake::Off
        };
        result
    }
}

impl Default for KeepAwake {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        let _ = self.inner.set(PerformanceKeepAwake::Off);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints what this computer reports. Run by hand:
    /// `cargo test -p platform performance_probe -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the real power state; prints it for a manual check"]
    fn performance_probe() {
        let state = read_state();
        eprintln!("{state:#?}");
        eprintln!("current supply: {:?}", current_supply());
    }

    /// Creates and releases a keep-awake request. Run by hand with `--ignored`; check
    /// `pmset -g assertions` (macOS) while it sleeps.
    #[test]
    #[ignore = "holds a real keep-awake assertion for a few seconds"]
    fn keep_awake_probe() {
        let mut k = KeepAwake::new();
        k.set(PerformanceKeepAwake::Display)
            .expect("display assertion");
        assert_eq!(k.current(), PerformanceKeepAwake::Display);
        eprintln!("holding PreventUserIdleDisplaySleep for 4 s");
        std::thread::sleep(std::time::Duration::from_secs(4));
        k.set(PerformanceKeepAwake::System)
            .expect("system assertion");
        eprintln!("holding PreventUserIdleSystemSleep for 4 s");
        std::thread::sleep(std::time::Duration::from_secs(4));
        k.set(PerformanceKeepAwake::Off).expect("release");
        eprintln!("released");
    }
}
