//! Windows: `Win32_Fan` (WMI, read only) and the "System cooling policy" power setting
//! (`GUID_SYSTEM_COOLING_POLICY` in the processor subgroup) of the active scheme, through
//! the documented powrprof API. Compile-checked from macOS; not yet run on Windows.

use std::sync::mpsc;
use std::time::Duration;

use serde::Deserialize;
use windows::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HLOCAL, LocalFree, WIN32_ERROR,
};
use windows::Win32::System::Power::{
    PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex, PowerSetActiveScheme,
    PowerWriteACValueIndex, PowerWriteDCValueIndex,
};
use windows::core::GUID;
use wmi::WMIConnection;

use super::{FanProbe, FanReading, FanSource, PolicyError, PolicyValues};

/// `GUID_PROCESSOR_SETTINGS_SUBGROUP`.
static SUBGROUP_PROCESSOR: GUID = GUID::from_u128(0x5453_3251_82be_4824_96c1_47b6_0b74_0d00);
/// `GUID_SYSTEM_COOLING_POLICY` (0 passive, 1 active).
static SYSTEM_COOLING_POLICY: GUID = GUID::from_u128(0x94d3_a615_a899_4ac5_ae2b_e4d8_f634_367f);

const WMI_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32Fan {
    device_id: Option<String>,
    name: Option<String>,
    desired_speed: Option<u64>,
}

fn fans_blocking() -> Result<Vec<FanReading>, String> {
    let con = WMIConnection::new().map_err(|e| format!("connect ROOT\\CIMV2: {e}"))?;
    let rows: Vec<Win32Fan> = con
        .raw_query("SELECT DeviceID, Name, DesiredSpeed FROM Win32_Fan")
        .map_err(|e| format!("Win32_Fan: {e}"))?;
    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(i, r)| FanReading {
            id: r
                .device_id
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("wmi-{i}")),
            label: r.name.filter(|s| !s.trim().is_empty()),
            // DesiredSpeed is the requested speed in RPM; Windows reports no actual speed.
            target_rpm: r.desired_speed.filter(|&v| v > 0).map(|v| v as f64),
            ..FanReading::default()
        })
        .collect())
}

pub fn probe_fans() -> FanProbe {
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("wmi-fans".into())
        .spawn(move || {
            let _ = tx.send(fans_blocking());
        });
    let result = match spawned {
        Ok(_) => rx
            .recv_timeout(WMI_TIMEOUT)
            .unwrap_or_else(|_| Err("Win32_Fan query timed out".to_owned())),
        Err(e) => Err(format!("could not spawn WMI thread: {e}")),
    };
    match result {
        Ok(fans) => FanProbe::Read {
            source: FanSource::Wmi,
            declared: Some(fans.len() as u32),
            fans,
        },
        Err(detail) => FanProbe::Unavailable {
            source: FanSource::Wmi,
            detail,
        },
    }
}

fn map_err(context: &str, code: u32) -> PolicyError {
    if code == ERROR_ACCESS_DENIED.0 {
        PolicyError::AccessDenied
    } else if code == ERROR_FILE_NOT_FOUND.0 {
        PolicyError::NotFound
    } else {
        PolicyError::Failed(format!("{context} failed (error {code})"))
    }
}

/// The active scheme's GUID, freed with `LocalFree` on drop.
struct ActiveScheme(*mut GUID);

impl ActiveScheme {
    fn get() -> Result<Self, PolicyError> {
        let mut ptr: *mut GUID = std::ptr::null_mut();
        // SAFETY: `ptr` receives a LocalAlloc'd GUID owned by the returned guard.
        let r = unsafe { PowerGetActiveScheme(None, &raw mut ptr) };
        if r != ERROR_SUCCESS || ptr.is_null() {
            return Err(map_err("PowerGetActiveScheme", r.0));
        }
        Ok(Self(ptr))
    }
}

impl Drop for ActiveScheme {
    fn drop(&mut self) {
        // SAFETY: allocated by PowerGetActiveScheme, freed once.
        let _ = unsafe { LocalFree(Some(HLOCAL(self.0.cast()))) };
    }
}

pub fn read_policy() -> Result<PolicyValues, PolicyError> {
    let scheme = ActiveScheme::get()?;
    let mut ac = 0u32;
    let mut dc = 0u32;
    // SAFETY: the GUID pointers are valid for the call; outputs are writable u32s.
    let r_ac = unsafe {
        PowerReadACValueIndex(
            None,
            Some(scheme.0),
            Some(&raw const SUBGROUP_PROCESSOR),
            Some(&raw const SYSTEM_COOLING_POLICY),
            &raw mut ac,
        )
    };
    if r_ac != ERROR_SUCCESS {
        return Err(map_err("PowerReadACValueIndex", r_ac.0));
    }
    // SAFETY: as above.
    let r_dc = unsafe {
        PowerReadDCValueIndex(
            None,
            Some(scheme.0),
            Some(&raw const SUBGROUP_PROCESSOR),
            Some(&raw const SYSTEM_COOLING_POLICY),
            &raw mut dc,
        )
    };
    Ok(PolicyValues {
        ac: Some(ac),
        dc: (r_dc == ERROR_SUCCESS.0).then_some(dc),
    })
}

pub fn write_policy(values: PolicyValues) -> Result<(), PolicyError> {
    let scheme = ActiveScheme::get()?;
    if let Some(ac) = values.ac {
        // SAFETY: valid GUID pointers for the duration of the call.
        let r: WIN32_ERROR = unsafe {
            PowerWriteACValueIndex(
                None,
                scheme.0,
                Some(&raw const SUBGROUP_PROCESSOR),
                Some(&raw const SYSTEM_COOLING_POLICY),
                ac,
            )
        };
        if r != ERROR_SUCCESS {
            return Err(map_err("PowerWriteACValueIndex", r.0));
        }
    }
    if let Some(dc) = values.dc {
        // SAFETY: as above.
        let r = unsafe {
            PowerWriteDCValueIndex(
                None,
                scheme.0,
                Some(&raw const SUBGROUP_PROCESSOR),
                Some(&raw const SYSTEM_COOLING_POLICY),
                dc,
            )
        };
        if r != ERROR_SUCCESS.0 {
            return Err(map_err("PowerWriteDCValueIndex", r));
        }
    }
    // Re-applying the active scheme makes the new values take effect now.
    // SAFETY: valid GUID pointer.
    let r = unsafe { PowerSetActiveScheme(None, Some(scheme.0)) };
    if r != ERROR_SUCCESS {
        return Err(map_err("PowerSetActiveScheme", r.0));
    }
    Ok(())
}
