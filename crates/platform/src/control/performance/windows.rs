//! Windows: power mode overlay, per-supply values of the active power scheme, keep-awake.
//! Compiled for x86_64-pc-windows-msvc from macOS; not run on Windows yet.

use std::sync::mpsc;
use std::thread::JoinHandle;

use ::windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_SUCCESS, HLOCAL, LocalFree, WIN32_ERROR,
};
use ::windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use ::windows::Win32::System::Power::{
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, GetSystemPowerStatus,
    PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex, PowerSetActiveScheme,
    PowerWriteACValueIndex, PowerWriteDCValueIndex, SYSTEM_POWER_STATUS, SetThreadExecutionState,
};
use ::windows::Win32::UI::Shell::ShellExecuteW;
use ::windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use ::windows::core::{GUID, s, w};
use models::control::PowerSupply;
use models::control::performance::{
    PerformanceBoostMode, PerformanceControl, PerformanceKeepAwake, PerformanceMode,
    PerformanceOsSupply,
};
use models::{AppError, ErrorCode};

use super::{Access, OsPowerState, PerformanceAccess, WriteFailure};

// Power setting GUIDs (winnt.h).
const SUB_PROCESSOR: GUID = GUID::from_u128(0x54533251_82be_4824_96c1_47b60b740d00);
const PROCESSOR_MAX: GUID = GUID::from_u128(0xbc5038f7_23e0_4960_96da_33abaf5935ec);
const PROCESSOR_MIN: GUID = GUID::from_u128(0x893dee8e_2bef_41e0_89c6_b55d0929964c);
const BOOST_MODE: GUID = GUID::from_u128(0xbe337238_0d82_4146_a960_4f3749d470c7);
const SUB_VIDEO: GUID = GUID::from_u128(0x7516b95f_f776_4464_8c53_06167f40cc99);
const VIDEO_TIMEOUT: GUID = GUID::from_u128(0x3c0bc021_c8a8_4e07_a973_6b14cbcb2b7e);
const SUB_SLEEP: GUID = GUID::from_u128(0x238c9fa8_0aad_41ed_83f4_97be242c8f20);
const STANDBY_TIMEOUT: GUID = GUID::from_u128(0x29f6c1db_86da_48c5_9fdb_f2b67b1f44da);

// Power mode overlays (Settings > System > Power > Power mode).
const OVERLAY_EFFICIENCY: GUID = GUID::from_u128(0x961cc777_2547_4f9d_8174_7d86181b8a7a);
const OVERLAY_BALANCED: GUID = GUID::from_u128(0);
/// Windows 10 "Better performance" (the recommended middle position).
const OVERLAY_BETTER_PERFORMANCE: GUID = GUID::from_u128(0x381b4222_f694_41f0_9685_ff5bb260df2e);
const OVERLAY_PERFORMANCE: GUID = GUID::from_u128(0xded574b5_45a0_4f42_8737_46345c09c238);

pub(crate) fn overlay_mode(guid: &GUID) -> Option<PerformanceMode> {
    if *guid == OVERLAY_EFFICIENCY {
        Some(PerformanceMode::Silent)
    } else if *guid == OVERLAY_BALANCED || *guid == OVERLAY_BETTER_PERFORMANCE {
        Some(PerformanceMode::Balanced)
    } else if *guid == OVERLAY_PERFORMANCE {
        Some(PerformanceMode::HighPerformance)
    } else {
        None
    }
}

fn overlay_guid(mode: PerformanceMode) -> GUID {
    match mode {
        PerformanceMode::Silent => OVERLAY_EFFICIENCY,
        PerformanceMode::Balanced => OVERLAY_BALANCED,
        PerformanceMode::HighPerformance => OVERLAY_PERFORMANCE,
    }
}

type GetOverlayFn = unsafe extern "system" fn(*mut GUID) -> u32;
type SetOverlayFn = unsafe extern "system" fn(GUID) -> u32;

/// The overlay calls are exported by powrprof.dll but not declared by the SDK headers, so
/// they are looked up at run time: an older Windows without them reports the mode as
/// unavailable instead of failing to start.
fn overlay_fns() -> Option<(GetOverlayFn, SetOverlayFn)> {
    // SAFETY: fixed DLL name, loaded from System32 only; the module stays loaded (never
    // freed), so the function pointers remain valid for the process lifetime.
    unsafe {
        let module = LoadLibraryExW(w!("powrprof.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
        let get = GetProcAddress(module, s!("PowerGetEffectiveOverlayScheme"))?;
        let set = GetProcAddress(module, s!("PowerSetActiveOverlayScheme"))?;
        // SAFETY: both exports have these signatures (DWORD WINAPI f(GUID*) / f(GUID)).
        Some((
            std::mem::transmute::<unsafe extern "system" fn() -> isize, GetOverlayFn>(get),
            std::mem::transmute::<unsafe extern "system" fn() -> isize, SetOverlayFn>(set),
        ))
    }
}

fn effective_overlay() -> Option<Result<GUID, u32>> {
    let (get, _) = overlay_fns()?;
    let mut guid = GUID::zeroed();
    // SAFETY: `guid` is a writable GUID.
    let rc = unsafe { get(&mut guid) };
    Some(if rc == 0 { Ok(guid) } else { Err(rc) })
}

pub fn current_supply() -> (PowerSupply, bool) {
    let mut st = SYSTEM_POWER_STATUS::default();
    // SAFETY: `st` is a valid, writable SYSTEM_POWER_STATUS.
    if unsafe { GetSystemPowerStatus(&mut st) }.is_err() {
        return (PowerSupply::Ac, false);
    }
    let has_battery = crate::form_factor::battery_from_flag(st.BatteryFlag).unwrap_or(false);
    let supply = if has_battery && st.ACLineStatus == 0 {
        PowerSupply::Battery
    } else {
        PowerSupply::Ac
    };
    (supply, has_battery)
}

fn active_scheme() -> Option<GUID> {
    let mut ptr: *mut GUID = std::ptr::null_mut();
    // SAFETY: out pointer is valid; on success the OS allocates the GUID with LocalAlloc.
    let rc = unsafe { PowerGetActiveScheme(None, &mut ptr) };
    if rc != ERROR_SUCCESS || ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` came from PowerGetActiveScheme and points to a GUID.
    let guid = unsafe { *ptr };
    // SAFETY: allocated by the OS with LocalAlloc; freed exactly once.
    unsafe {
        let _ = LocalFree(Some(HLOCAL(ptr.cast())));
    }
    Some(guid)
}

fn read_index(scheme: &GUID, sub: &GUID, setting: &GUID, supply: PowerSupply) -> Option<u32> {
    let mut value = 0u32;
    // SAFETY: GUIDs live on the stack for the call; `value` is writable.
    let rc = unsafe {
        match supply {
            PowerSupply::Ac => {
                PowerReadACValueIndex(None, Some(scheme), Some(sub), Some(setting), &mut value).0
            }
            PowerSupply::Battery => {
                PowerReadDCValueIndex(None, Some(scheme), Some(sub), Some(setting), &mut value)
            }
        }
    };
    (rc == 0).then_some(value)
}

fn write_index(scheme: &GUID, sub: &GUID, setting: &GUID, supply: PowerSupply, value: u32) -> u32 {
    // SAFETY: GUIDs live on the stack for the call.
    unsafe {
        match supply {
            PowerSupply::Ac => {
                PowerWriteACValueIndex(None, scheme, Some(sub), Some(setting), value).0
            }
            PowerSupply::Battery => {
                PowerWriteDCValueIndex(None, scheme, Some(sub), Some(setting), value)
            }
        }
    }
}

fn read_supply(scheme: Option<&GUID>, supply: PowerSupply) -> PerformanceOsSupply {
    let Some(scheme) = scheme else {
        return PerformanceOsSupply::default();
    };
    let pct = |g: &GUID| {
        read_index(scheme, &SUB_PROCESSOR, g, supply).and_then(|v| u8::try_from(v.min(100)).ok())
    };
    PerformanceOsSupply {
        mode: None,
        processor_max_percent: pct(&PROCESSOR_MAX),
        processor_min_percent: pct(&PROCESSOR_MIN),
        boost_mode: read_index(scheme, &SUB_PROCESSOR, &BOOST_MODE, supply)
            .and_then(PerformanceBoostMode::from_index),
        display_off_seconds: read_index(scheme, &SUB_VIDEO, &VIDEO_TIMEOUT, supply),
        sleep_seconds: read_index(scheme, &SUB_SLEEP, &STANDBY_TIMEOUT, supply),
    }
}

pub fn read_state() -> OsPowerState {
    let (supply, has_battery) = current_supply();
    let scheme = active_scheme();
    let mut ac = read_supply(scheme.as_ref(), PowerSupply::Ac);
    let mut battery = has_battery.then(|| read_supply(scheme.as_ref(), PowerSupply::Battery));
    let overlay = effective_overlay();
    if let Some(Ok(guid)) = &overlay {
        let current = match supply {
            PowerSupply::Ac => &mut ac,
            PowerSupply::Battery => battery.as_mut().unwrap_or(&mut ac),
        };
        current.mode = overlay_mode(guid);
    }
    let readable = |v: bool| {
        if v {
            Access::Writable
        } else {
            Access::NotAvailable
        }
    };
    let access = PerformanceAccess {
        mode: readable(matches!(overlay, Some(Ok(_)))),
        keep_awake: Access::Writable,
        processor_state: readable(ac.processor_max_percent.is_some()),
        boost_mode: readable(ac.boost_mode.is_some()),
        display_timeout: readable(ac.display_off_seconds.is_some()),
        sleep_timeout: readable(ac.sleep_seconds.is_some()),
    };
    OsPowerState {
        supply,
        has_battery,
        ac,
        battery,
        scheme_id: scheme.map(|g| format!("{g:?}")),
        scheme_name: crate::windows::bench_env::active_power_plan(),
        access,
    }
}

fn failure(control: PerformanceControl, call: &str, rc: u32) -> WriteFailure {
    WriteFailure {
        control,
        denied: WIN32_ERROR(rc) == ERROR_ACCESS_DENIED,
        detail: format!("{call} failed with error {rc}"),
    }
}

pub fn write_settings(
    scheme_id: Option<&str>,
    current: PowerSupply,
    ac: &PerformanceOsSupply,
    battery: Option<&PerformanceOsSupply>,
) -> Vec<WriteFailure> {
    let mut failures = Vec::new();
    let scheme = match scheme_id {
        Some(id) => GUID::try_from(id).ok(),
        None => active_scheme(),
    };
    let mut wrote = false;
    if let Some(scheme) = scheme {
        let supplies = std::iter::once((PowerSupply::Ac, ac))
            .chain(battery.map(|b| (PowerSupply::Battery, b)));
        for (supply, values) in supplies {
            let writes: [(PerformanceControl, &GUID, &GUID, Option<u32>); 5] = [
                (
                    PerformanceControl::ProcessorState,
                    &SUB_PROCESSOR,
                    &PROCESSOR_MAX,
                    values.processor_max_percent.map(u32::from),
                ),
                (
                    PerformanceControl::ProcessorState,
                    &SUB_PROCESSOR,
                    &PROCESSOR_MIN,
                    values.processor_min_percent.map(u32::from),
                ),
                (
                    PerformanceControl::BoostMode,
                    &SUB_PROCESSOR,
                    &BOOST_MODE,
                    values.boost_mode.map(PerformanceBoostMode::index),
                ),
                (
                    PerformanceControl::DisplayTimeout,
                    &SUB_VIDEO,
                    &VIDEO_TIMEOUT,
                    values.display_off_seconds,
                ),
                (
                    PerformanceControl::SleepTimeout,
                    &SUB_SLEEP,
                    &STANDBY_TIMEOUT,
                    values.sleep_seconds,
                ),
            ];
            for (control, sub, setting, value) in writes {
                let Some(value) = value else { continue };
                let rc = write_index(&scheme, sub, setting, supply, value);
                if rc == 0 {
                    wrote = true;
                } else {
                    failures.push(failure(control, "PowerWriteValueIndex", rc));
                }
            }
        }
        // Written values take effect when the scheme is (re)activated.
        let restore_other = scheme_id.is_some() && active_scheme() != Some(scheme);
        if wrote || restore_other {
            // SAFETY: `scheme` lives on the stack for the call.
            let rc = unsafe { PowerSetActiveScheme(None, Some(&scheme)) }.0;
            if rc != 0 {
                failures.push(failure(
                    PerformanceControl::ProcessorState,
                    "PowerSetActiveScheme",
                    rc,
                ));
            }
        }
    }

    let mode = match current {
        PowerSupply::Ac => ac.mode,
        PowerSupply::Battery => battery.and_then(|b| b.mode),
    };
    if let Some(mode) = mode {
        match overlay_fns() {
            Some((_, set)) => {
                // SAFETY: the GUID is passed by value.
                let rc = unsafe { set(overlay_guid(mode)) };
                if rc != 0 {
                    failures.push(failure(
                        PerformanceControl::Mode,
                        "PowerSetActiveOverlayScheme",
                        rc,
                    ));
                }
            }
            None => failures.push(WriteFailure {
                control: PerformanceControl::Mode,
                denied: false,
                detail: "PowerSetActiveOverlayScheme is not available".to_owned(),
            }),
        }
    }
    failures.dedup_by(|a, b| a.control == b.control);
    failures
}

pub fn open_power_settings() -> Result<(), AppError> {
    // SAFETY: fixed, NUL-terminated wide strings.
    let h = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("ms-settings:powersleep"),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    // ShellExecute returns a value greater than 32 on success.
    if h.0 as usize > 32 {
        Ok(())
    } else {
        Err(
            AppError::new(ErrorCode::Platform, "could not open Windows Settings")
                .with_detail(format!("ShellExecuteW returned {}", h.0 as usize)),
        )
    }
}

enum Request {
    Set(PerformanceKeepAwake, mpsc::Sender<Result<(), AppError>>),
    Stop,
}

/// `SetThreadExecutionState` is per thread: a dedicated thread holds the request for as
/// long as it is set.
#[derive(Default)]
pub struct KeepAwakeImpl {
    worker: Option<(mpsc::Sender<Request>, JoinHandle<()>)>,
}

fn worker_loop(rx: &mpsc::Receiver<Request>) {
    while let Ok(req) = rx.recv() {
        match req {
            Request::Set(mode, reply) => {
                let flags = match mode {
                    PerformanceKeepAwake::Off => ES_CONTINUOUS,
                    PerformanceKeepAwake::System => ES_CONTINUOUS | ES_SYSTEM_REQUIRED,
                    PerformanceKeepAwake::Display => {
                        ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
                    }
                };
                // SAFETY: plain flags.
                let previous = unsafe { SetThreadExecutionState(flags) };
                let _ = reply.send(if previous.0 == 0 {
                    Err(AppError::new(
                        ErrorCode::Platform,
                        "SetThreadExecutionState failed",
                    ))
                } else {
                    Ok(())
                });
            }
            Request::Stop => break,
        }
    }
    // SAFETY: plain flags; clears this thread's request before it exits.
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS);
    }
}

impl KeepAwakeImpl {
    pub fn set(&mut self, mode: PerformanceKeepAwake) -> Result<(), AppError> {
        if self.worker.is_none() {
            if mode == PerformanceKeepAwake::Off {
                return Ok(());
            }
            let (tx, rx) = mpsc::channel();
            let handle = std::thread::Builder::new()
                .name("performance-keep-awake".into())
                .spawn(move || worker_loop(&rx))
                .map_err(|e| {
                    AppError::internal("could not start the keep-awake thread")
                        .with_detail(e.to_string())
                })?;
            self.worker = Some((tx, handle));
        }
        let Some((tx, _)) = &self.worker else {
            return Ok(());
        };
        let (reply_tx, reply_rx) = mpsc::channel();
        tx.send(Request::Set(mode, reply_tx))
            .map_err(|_| AppError::internal("keep-awake thread stopped"))?;
        reply_rx
            .recv()
            .map_err(|_| AppError::internal("keep-awake thread stopped"))?
    }
}

impl Drop for KeepAwakeImpl {
    fn drop(&mut self) {
        if let Some((tx, handle)) = self.worker.take() {
            let _ = tx.send(Request::Stop);
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlays_map_to_modes() {
        for mode in [
            PerformanceMode::Silent,
            PerformanceMode::Balanced,
            PerformanceMode::HighPerformance,
        ] {
            assert_eq!(overlay_mode(&overlay_guid(mode)), Some(mode));
        }
        assert_eq!(
            overlay_mode(&OVERLAY_BETTER_PERFORMANCE),
            Some(PerformanceMode::Balanced)
        );
        assert_eq!(overlay_mode(&SUB_VIDEO), None);
    }

    #[test]
    fn scheme_ids_round_trip() {
        let id = format!("{SUB_PROCESSOR:?}");
        assert_eq!(GUID::try_from(id.as_str()).ok(), Some(SUB_PROCESSOR));
    }
}
