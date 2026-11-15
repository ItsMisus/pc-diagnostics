//! macOS: power preferences per supply (read only without root) and keep-awake assertions.

use std::ffi::c_void;
use std::ptr::NonNull;

use models::control::PowerSupply;
use models::control::performance::{
    PerformanceControl, PerformanceKeepAwake, PerformanceMode, PerformanceOsSupply,
};
use models::{AppError, ErrorCode};
use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_io_kit::{
    IOPMAssertionCreateWithName, IOPMAssertionID, IOPMAssertionRelease, kIOPMAssertionLevelOn,
    kIOReturnSuccess,
};

use super::{Access, OsPowerState, PerformanceAccess, WriteFailure};

const AC_KEY: &str = "AC Power";
const BATTERY_KEY: &str = "Battery Power";
/// System Settings > Battery (on Macs without a battery the same pane is "Energy").
const SETTINGS_URL: &str = "x-apple.systempreferences:com.apple.Battery-Settings.extension";

// Exported by IOKit.framework (used by `pmset`); no entitlement. Declared here because the
// header is not part of the SDK. Both only read.
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    /// +1 `CFDictionaryRef` keyed by power source name, or NULL.
    fn IOPMCopyPMPreferences() -> *mut c_void;
    fn IOPMFeatureIsAvailable(feature: *const c_void, power_source: *const c_void) -> u8;
}

fn cf_ptr(s: &CFString) -> *const c_void {
    (s as *const CFString).cast()
}

fn feature_available(feature: &str, source: &str) -> bool {
    let f = CFString::from_str(feature);
    let s = CFString::from_str(source);
    // SAFETY: both arguments are valid CFStrings alive for the call; the call only reads.
    unsafe { IOPMFeatureIsAvailable(cf_ptr(&f), cf_ptr(&s)) != 0 }
}

/// The PM preferences: one dictionary per power source, values are CF numbers.
struct Prefs(CFRetained<CFDictionary>);

impl Prefs {
    fn copy() -> Option<Self> {
        // SAFETY: no arguments; the result follows the Create rule (+1) or is NULL.
        let raw = unsafe { IOPMCopyPMPreferences() };
        let ptr = NonNull::new(raw.cast::<CFDictionary>())?;
        // SAFETY: `ptr` is a +1 CFDictionary reference we now own.
        Some(Self(unsafe { CFRetained::from_raw(ptr) }))
    }

    fn source(&self, key: &str) -> Option<CFRetained<CFDictionary>> {
        // SAFETY: the preferences dictionary has CFString keys (power source names) and CF
        // object values, so `CFType` is a valid view.
        let typed = unsafe { self.0.cast_unchecked::<CFString, CFType>() };
        typed
            .get(&CFString::from_str(key))?
            .downcast::<CFDictionary>()
            .ok()
    }
}

fn number(dict: &CFDictionary, key: &str) -> Option<i64> {
    // SAFETY: per-source preference dictionaries have CFString keys and CF values.
    let typed = unsafe { dict.cast_unchecked::<CFString, CFType>() };
    typed
        .get(&CFString::from_str(key))?
        .downcast_ref::<CFNumber>()?
        .as_i64()
}

/// Mode from one source's preferences: `PowerMode` (newer Macs with High Power Mode:
/// 0 automatic, 1 low, 2 high), else `HighPowerMode` / `LowPowerMode` flags.
pub(crate) fn mode_from(
    power_mode: Option<i64>,
    high: Option<i64>,
    low: Option<i64>,
) -> Option<PerformanceMode> {
    match power_mode {
        Some(1) => return Some(PerformanceMode::Silent),
        Some(2) => return Some(PerformanceMode::HighPerformance),
        Some(0) => return Some(PerformanceMode::Balanced),
        _ => {}
    }
    if high == Some(1) {
        return Some(PerformanceMode::HighPerformance);
    }
    match low {
        Some(1) => Some(PerformanceMode::Silent),
        Some(0) => Some(PerformanceMode::Balanced),
        _ => None,
    }
}

fn minutes_to_seconds(m: Option<i64>) -> Option<u32> {
    m.and_then(|m| u32::try_from(m).ok())
        .map(|m| m.saturating_mul(60))
}

fn supply_from(dict: &CFDictionary) -> PerformanceOsSupply {
    PerformanceOsSupply {
        mode: mode_from(
            number(dict, "PowerMode"),
            number(dict, "HighPowerMode"),
            number(dict, "LowPowerMode"),
        ),
        processor_max_percent: None,
        processor_min_percent: None,
        boost_mode: None,
        display_off_seconds: minutes_to_seconds(number(dict, "Display Sleep Timer")),
        sleep_seconds: minutes_to_seconds(number(dict, "System Sleep Timer")),
    }
}

pub fn current_supply() -> (PowerSupply, bool) {
    let has_battery = crate::macos::device::internal_battery()
        .value()
        .copied()
        .unwrap_or(false);
    let supply = match crate::macos::bench_env::power_source() {
        models::benchmark_run::PowerSource::Battery if has_battery => PowerSupply::Battery,
        _ => PowerSupply::Ac,
    };
    (supply, has_battery)
}

pub fn read_state() -> OsPowerState {
    let (supply, has_battery) = current_supply();
    let prefs = Prefs::copy();
    let mut ac = prefs
        .as_ref()
        .and_then(|p| p.source(AC_KEY))
        .map(|d| supply_from(&d))
        .unwrap_or_default();
    let mut battery = has_battery.then(|| {
        prefs
            .as_ref()
            .and_then(|p| p.source(BATTERY_KEY))
            .map(|d| supply_from(&d))
            .unwrap_or_default()
    });
    // Without the preferences, the effective Low Power Mode still tells the current mode.
    let current = match supply {
        PowerSupply::Ac => &mut ac,
        PowerSupply::Battery => battery.as_mut().unwrap_or(&mut ac),
    };
    if current.mode.is_none() {
        let low = objc2_foundation::NSProcessInfo::processInfo().isLowPowerModeEnabled();
        current.mode = Some(if low {
            PerformanceMode::Silent
        } else {
            PerformanceMode::Balanced
        });
    }

    let source = if has_battery { BATTERY_KEY } else { AC_KEY };
    let mode =
        if feature_available("LowPowerMode", source) || feature_available("LowPowerMode", AC_KEY) {
            Access::NeedsAdmin
        } else {
            Access::NotAvailable
        };
    OsPowerState {
        supply,
        has_battery,
        ac,
        battery,
        scheme_id: None,
        scheme_name: None,
        access: PerformanceAccess {
            mode,
            keep_awake: Access::Writable,
            processor_state: Access::NotOnThisOs,
            boost_mode: Access::NotOnThisOs,
            display_timeout: Access::NeedsAdmin,
            sleep_timeout: Access::NeedsAdmin,
        },
    }
}

/// Nothing but keep-awake is writable without root on macOS: any value passed is refused.
pub fn write_settings(
    _scheme_id: Option<&str>,
    current: PowerSupply,
    ac: &PerformanceOsSupply,
    battery: Option<&PerformanceOsSupply>,
) -> Vec<WriteFailure> {
    let mut failures = Vec::new();
    let mode = match current {
        PowerSupply::Ac => ac.mode,
        PowerSupply::Battery => battery.and_then(|b| b.mode),
    };
    let mut refuse = |control, set: bool| {
        if set {
            failures.push(WriteFailure {
                control,
                denied: true,
                detail: "macOS power preferences need administrator rights".to_owned(),
            });
        }
    };
    refuse(PerformanceControl::Mode, mode.is_some());
    for s in std::iter::once(ac).chain(battery) {
        refuse(
            PerformanceControl::ProcessorState,
            s.processor_max_percent.is_some() || s.processor_min_percent.is_some(),
        );
        refuse(PerformanceControl::BoostMode, s.boost_mode.is_some());
        refuse(
            PerformanceControl::DisplayTimeout,
            s.display_off_seconds.is_some(),
        );
        refuse(PerformanceControl::SleepTimeout, s.sleep_seconds.is_some());
    }
    failures.dedup_by_key(|f| f.control);
    failures
}

pub fn open_power_settings() -> Result<(), AppError> {
    let status = std::process::Command::new("/usr/bin/open")
        .arg(SETTINGS_URL)
        .status()
        .map_err(|e| {
            AppError::new(ErrorCode::Platform, "could not open System Settings")
                .with_detail(e.to_string())
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(
            AppError::new(ErrorCode::Platform, "could not open System Settings")
                .with_detail(format!("open exited with {status}")),
        )
    }
}

/// One I/O Kit power assertion at a time.
#[derive(Default)]
pub struct KeepAwakeImpl {
    id: Option<IOPMAssertionID>,
}

impl KeepAwakeImpl {
    pub fn set(&mut self, mode: PerformanceKeepAwake) -> Result<(), AppError> {
        if let Some(id) = self.id.take() {
            let rc = IOPMAssertionRelease(id);
            if rc != kIOReturnSuccess {
                tracing::warn!(rc, "IOPMAssertionRelease failed");
            }
        }
        let kind = match mode {
            PerformanceKeepAwake::Off => return Ok(()),
            PerformanceKeepAwake::System => "PreventUserIdleSystemSleep",
            PerformanceKeepAwake::Display => "PreventUserIdleDisplaySleep",
        };
        let kind = CFString::from_str(kind);
        let name = CFString::from_str("PC Diagnostics performance profile");
        let mut id: IOPMAssertionID = 0;
        // SAFETY: valid CFStrings alive for the call; `id` is a writable assertion id.
        let rc = unsafe {
            IOPMAssertionCreateWithName(Some(&kind), kIOPMAssertionLevelOn, Some(&name), &mut id)
        };
        if rc != kIOReturnSuccess {
            return Err(
                AppError::new(ErrorCode::Platform, "IOPMAssertionCreateWithName failed")
                    .with_detail(format!("IOReturn 0x{rc:08X}")),
            );
        }
        self.id = Some(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_from_preferences() {
        assert_eq!(
            mode_from(None, None, Some(1)),
            Some(PerformanceMode::Silent)
        );
        assert_eq!(
            mode_from(None, None, Some(0)),
            Some(PerformanceMode::Balanced)
        );
        assert_eq!(
            mode_from(None, Some(1), Some(0)),
            Some(PerformanceMode::HighPerformance)
        );
        assert_eq!(
            mode_from(Some(2), None, Some(0)),
            Some(PerformanceMode::HighPerformance)
        );
        assert_eq!(
            mode_from(Some(1), None, None),
            Some(PerformanceMode::Silent)
        );
        assert_eq!(mode_from(None, None, None), None);
    }

    #[test]
    fn preferences_are_readable_without_privileges() {
        // Every Mac running the app has PM preferences for mains power.
        let state = read_state();
        assert!(state.ac.mode.is_some());
        assert_eq!(state.access.keep_awake, Access::Writable);
    }

    #[test]
    fn writes_are_refused_with_a_reason() {
        let values = PerformanceOsSupply {
            mode: Some(PerformanceMode::Silent),
            sleep_seconds: Some(60),
            ..PerformanceOsSupply::default()
        };
        let f = write_settings(None, PowerSupply::Ac, &values, None);
        assert_eq!(
            f.iter().map(|f| f.control).collect::<Vec<_>>(),
            vec![PerformanceControl::Mode, PerformanceControl::SleepTimeout]
        );
        assert!(f.iter().all(|f| f.denied));
    }
}
