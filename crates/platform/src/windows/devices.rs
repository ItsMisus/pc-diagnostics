//! Present-device enumeration via SetupAPI + CfgMgr32. Read-only: the device information
//! set is opened for querying only and nothing is ever written to a device.
//!
//! No COM is involved, so this runs on the caller's thread. Cost is a few milliseconds per
//! hundred devices.

use ::windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_DEVNODE_STATUS_FLAGS, CM_Get_DevNode_Status, CM_PROB, CR_SUCCESS, DIGCF_ALLCLASSES,
    DIGCF_PRESENT, DN_HAS_PROBLEM, DN_STARTED, HDEVINFO, SP_DEVINFO_DATA,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
    SetupDiGetDeviceInstanceIdW, SetupDiGetDevicePropertyW,
};
use ::windows::Win32::Devices::Properties::{
    DEVPKEY_Device_Class, DEVPKEY_Device_DeviceDesc, DEVPKEY_Device_DriverDate,
    DEVPKEY_Device_DriverInfPath, DEVPKEY_Device_DriverProvider, DEVPKEY_Device_DriverVersion,
    DEVPKEY_Device_FriendlyName, DEVPKEY_Device_HardwareIds, DEVPKEY_Device_Manufacturer,
    DEVPROP_TYPE_FILETIME, DEVPROP_TYPE_STRING, DEVPROP_TYPE_STRING_LIST, DEVPROPTYPE,
};
use ::windows::Win32::Foundation::{
    DEVPROPKEY, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, ERROR_NOT_FOUND,
};
use ::windows::core::PCWSTR;
use models::{AppError, ErrorCode};

use crate::devices::{
    DevNodeStatus, RawDevice, bytes_to_u16, filetime_to_iso_date, format_guid, parse_multi_sz,
    utf16_to_string,
};

/// Owns an `HDEVINFO` and destroys it on drop.
struct DevInfoSet(HDEVINFO);

impl Drop for DevInfoSet {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful SetupDiGetClassDevsW and is destroyed
        // exactly once, here.
        if let Err(e) = unsafe { SetupDiDestroyDeviceInfoList(self.0) } {
            tracing::warn!(error = %e, "SetupDiDestroyDeviceInfoList failed");
        }
    }
}

fn win32_error(op: &str, e: &::windows::core::Error) -> AppError {
    AppError::new(ErrorCode::Platform, format!("{op} failed")).with_detail(format!(
        "HRESULT 0x{:08X}: {}",
        e.code().0,
        e.message()
    ))
}

/// Raw property bytes, or `None` when the device does not have the property.
fn property(
    set: &DevInfoSet,
    data: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
) -> Option<(DEVPROPTYPE, Vec<u8>)> {
    let mut buf = vec![0u8; 256];
    for _ in 0..3 {
        let mut ty = DEVPROPTYPE::default();
        let mut required = 0u32;
        // SAFETY: `set` is a live device information set, `data` was filled by
        // SetupDiEnumDeviceInfo on that set, `key` points to a static DEVPROPKEY, and the
        // output buffer slice carries its own length.
        let r = unsafe {
            SetupDiGetDevicePropertyW(
                set.0,
                data,
                key,
                &mut ty,
                Some(buf.as_mut_slice()),
                Some(&mut required),
                0,
            )
        };
        match r {
            Ok(()) => {
                buf.truncate(required as usize);
                return Some((ty, buf));
            }
            Err(e) if e.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() => {
                buf = vec![0u8; (required as usize).max(buf.len() * 2)];
            }
            Err(e) if e.code() == ERROR_NOT_FOUND.to_hresult() => return None,
            Err(e) => {
                tracing::debug!(error = %e, "SetupDiGetDevicePropertyW failed");
                return None;
            }
        }
    }
    None
}

fn string_property(set: &DevInfoSet, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<String> {
    match property(set, data, key)? {
        (ty, bytes) if ty == DEVPROP_TYPE_STRING => utf16_to_string(&bytes_to_u16(&bytes)),
        _ => None,
    }
}

fn string_list_property(set: &DevInfoSet, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Vec<String> {
    match property(set, data, key) {
        Some((ty, bytes)) if ty == DEVPROP_TYPE_STRING_LIST => {
            parse_multi_sz(&bytes_to_u16(&bytes))
        }
        Some((ty, bytes)) if ty == DEVPROP_TYPE_STRING => {
            utf16_to_string(&bytes_to_u16(&bytes)).into_iter().collect()
        }
        _ => Vec::new(),
    }
}

fn filetime_property(set: &DevInfoSet, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<String> {
    match property(set, data, key)? {
        (ty, bytes) if ty == DEVPROP_TYPE_FILETIME && bytes.len() >= 8 => {
            let mut raw = [0u8; 8];
            raw.copy_from_slice(&bytes[..8]);
            // FILETIME is { dwLowDateTime, dwHighDateTime }: a little-endian u64.
            filetime_to_iso_date(u64::from_le_bytes(raw))
        }
        _ => None,
    }
}

fn instance_id(set: &DevInfoSet, data: &SP_DEVINFO_DATA) -> Result<String, AppError> {
    let mut buf = vec![0u16; 256];
    for _ in 0..3 {
        let mut required = 0u32;
        // SAFETY: live set and enumerated element; the slice carries its own length.
        let r = unsafe {
            SetupDiGetDeviceInstanceIdW(set.0, data, Some(buf.as_mut_slice()), Some(&mut required))
        };
        match r {
            Ok(()) => {
                return utf16_to_string(&buf).ok_or_else(|| {
                    AppError::new(ErrorCode::Platform, "device has an empty instance id")
                });
            }
            Err(e) if e.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() => {
                buf = vec![0u16; (required as usize).max(buf.len() * 2)];
            }
            Err(e) => return Err(win32_error("SetupDiGetDeviceInstanceIdW", &e)),
        }
    }
    Err(AppError::new(
        ErrorCode::Platform,
        "SetupDiGetDeviceInstanceIdW buffer too small after retries",
    ))
}

fn devnode_status(devinst: u32) -> Result<DevNodeStatus, String> {
    let mut status = CM_DEVNODE_STATUS_FLAGS::default();
    let mut problem = CM_PROB::default();
    // SAFETY: both out-pointers reference live locals; `devinst` comes from SetupAPI.
    let cr = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, devinst, 0) };
    if cr != CR_SUCCESS {
        return Err(format!("CM_Get_DevNode_Status returned CONFIGRET {}", cr.0));
    }
    let has_problem = status.0 & DN_HAS_PROBLEM.0 != 0;
    Ok(DevNodeStatus {
        started: status.0 & DN_STARTED.0 != 0,
        problem_code: has_problem.then_some(problem.0),
    })
}

/// Every present device of every setup class.
pub fn enumerate_present() -> Result<Vec<RawDevice>, AppError> {
    // SAFETY: no class GUID / enumerator / window; flags request present devices of all
    // classes. The returned handle is owned by `DevInfoSet` right away.
    let handle = unsafe {
        SetupDiGetClassDevsW(None, PCWSTR::null(), None, DIGCF_PRESENT | DIGCF_ALLCLASSES)
    }
    .map_err(|e| win32_error("SetupDiGetClassDevsW", &e))?;
    let set = DevInfoSet(handle);

    let mut out = Vec::new();
    for index in 0u32.. {
        let mut data = SP_DEVINFO_DATA {
            cbSize: u32::try_from(std::mem::size_of::<SP_DEVINFO_DATA>()).unwrap_or(0),
            ..Default::default()
        };
        // SAFETY: live set; `data.cbSize` is initialized as the API requires.
        match unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut data) } {
            Ok(()) => {}
            Err(e) if e.code() == ERROR_NO_MORE_ITEMS.to_hresult() => break,
            Err(e) => return Err(win32_error("SetupDiEnumDeviceInfo", &e)),
        }
        let instance_id = match instance_id(&set, &data) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(index, error = %e, "device without instance id skipped");
                continue;
            }
        };
        let g = data.ClassGuid;
        out.push(RawDevice {
            instance_id,
            friendly_name: string_property(&set, &data, &DEVPKEY_Device_FriendlyName),
            description: string_property(&set, &data, &DEVPKEY_Device_DeviceDesc),
            manufacturer: string_property(&set, &data, &DEVPKEY_Device_Manufacturer),
            class_name: string_property(&set, &data, &DEVPKEY_Device_Class),
            class_guid: format_guid(g.data1, g.data2, g.data3, g.data4),
            hardware_ids: string_list_property(&set, &data, &DEVPKEY_Device_HardwareIds),
            status: devnode_status(data.DevInst),
            driver_version: string_property(&set, &data, &DEVPKEY_Device_DriverVersion),
            driver_date: filetime_property(&set, &data, &DEVPKEY_Device_DriverDate),
            driver_provider: string_property(&set, &data, &DEVPKEY_Device_DriverProvider),
            driver_inf: string_property(&set, &data, &DEVPKEY_Device_DriverInfPath),
        });
    }
    Ok(out)
}
