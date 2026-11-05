//! Windows implementation of `crate::bench_env`. Read-only queries only.

use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::sync::mpsc;
use std::time::Duration;

use ::windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree};
use ::windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDiskFreeSpaceW, GetDriveTypeW, GetLogicalDrives,
    IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
};
use ::windows::Win32::System::IO::DeviceIoControl;
use ::windows::Win32::System::Ioctl::DISK_EXTENT;
use ::windows::Win32::System::Power::{
    GetSystemPowerStatus, PowerGetActiveScheme, PowerReadFriendlyName, SYSTEM_POWER_STATUS,
};
use ::windows::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
use ::windows::Win32::System::WindowsProgramming::DRIVE_FIXED;
use ::windows::core::{GUID, PCWSTR};
use models::benchmark_run::PowerSource;
use models::metric::{Metric, Source, Unavailability, Unit};
use models::{AppError, ErrorCode};

use super::power;
use crate::bench_env::VolumeSpace;
use crate::convert::average_nonzero;

/// WMI can stall; after this the disk model is reported as unknown.
const WMI_TIMEOUT: Duration = Duration::from_secs(10);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn power_source() -> PowerSource {
    let mut st = SYSTEM_POWER_STATUS::default();
    // SAFETY: `st` is a valid, writable SYSTEM_POWER_STATUS.
    if unsafe { GetSystemPowerStatus(&mut st) }.is_err() {
        return PowerSource::Unknown;
    }
    match st.ACLineStatus {
        1 => PowerSource::Ac,
        0 => PowerSource::Battery,
        _ => PowerSource::Unknown,
    }
}

pub fn active_power_plan() -> Option<String> {
    let mut guid_ptr: *mut GUID = std::ptr::null_mut();
    // SAFETY: out pointer is valid; on success the OS allocates the GUID with LocalAlloc.
    let rc = unsafe { PowerGetActiveScheme(None, &mut guid_ptr) };
    if rc != ERROR_SUCCESS || guid_ptr.is_null() {
        return None;
    }
    // SAFETY: `guid_ptr` was returned by PowerGetActiveScheme and is a valid GUID.
    let guid = unsafe { *guid_ptr };
    // SAFETY: the buffer was allocated by the OS with LocalAlloc; freed exactly once.
    unsafe {
        let _ = LocalFree(Some(HLOCAL(guid_ptr.cast())));
    }

    let mut size: u32 = 0;
    // SAFETY: size query (no buffer); `guid` lives on the stack for the call.
    let rc = unsafe { PowerReadFriendlyName(None, Some(&guid), None, None, None, &mut size) };
    if rc != ERROR_SUCCESS || size == 0 || size > 64 * 1024 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    let mut size_bytes = u32::try_from(buf.len() * 2).ok()?;
    // SAFETY: `buf` is writable and `size_bytes` long.
    let rc = unsafe {
        PowerReadFriendlyName(
            None,
            Some(&guid),
            None,
            None,
            Some(buf.as_mut_ptr().cast()),
            &mut size_bytes,
        )
    };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    let name = String::from_utf16_lossy(&buf[..end]).trim().to_owned();
    (!name.is_empty()).then_some(name)
}

pub fn cpu_frequency_limits() -> (Metric<f64>, Metric<f64>) {
    match power::processor_power_info() {
        Ok(info) => {
            let m = |v: Option<f64>| match v {
                Some(v) => Metric::available(v, Unit::Megahertz, Source::PowerApi),
                None => Metric::unavailable(Unavailability::NotSupported),
            };
            (
                m(average_nonzero(info.iter().map(|p| p.MaxMhz))),
                m(average_nonzero(info.iter().map(|p| p.MhzLimit))),
            )
        }
        Err(e) => {
            let r = Unavailability::Failed(e);
            (Metric::unavailable(r.clone()), Metric::unavailable(r))
        }
    }
}

pub fn fixed_volumes() -> Vec<String> {
    // SAFETY: no arguments.
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| format!("{}:\\", char::from(b'A' + i)))
        .filter(|root| {
            let w = wide(root);
            // SAFETY: `w` is a NUL-terminated UTF-16 string alive for the call.
            unsafe { GetDriveTypeW(PCWSTR(w.as_ptr())) == DRIVE_FIXED }
        })
        .collect()
}

pub fn system_volume() -> Option<String> {
    let mut buf = [0u16; 260];
    // SAFETY: `buf` is a writable slice; the length is passed by the wrapper.
    let n = unsafe { GetSystemWindowsDirectoryW(Some(&mut buf)) } as usize;
    if n == 0 || n > buf.len() {
        return None;
    }
    let dir = String::from_utf16_lossy(&buf[..n]);
    crate::bench_env::normalize_drive_mount(dir.get(..2)?)
}

pub fn volume_space(path: &str) -> Result<VolumeSpace, AppError> {
    let w = wide(path);
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: `w` is NUL-terminated; out pointers are valid u64s.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut free), Some(&mut total), None) }
        .map_err(|e| {
            AppError::new(ErrorCode::Platform, "GetDiskFreeSpaceExW failed")
                .with_detail(format!("{path}: {e}"))
        })?;
    Ok(VolumeSpace {
        free_to_caller_bytes: free,
        total_bytes: total,
    })
}

pub fn volume_sector_size(mount_point: &str) -> Option<u32> {
    let w = wide(mount_point);
    let mut bytes_per_sector = 0u32;
    // SAFETY: `w` is NUL-terminated; the out pointer is a valid u32.
    unsafe {
        GetDiskFreeSpaceW(
            PCWSTR(w.as_ptr()),
            None,
            Some(&mut bytes_per_sector),
            None,
            None,
        )
    }
    .ok()?;
    (bytes_per_sector > 0).then_some(bytes_per_sector)
}

/// `VOLUME_DISK_EXTENTS` with room for 16 extents (same layout: u32 count, then the
/// 8-byte-aligned extent array).
#[repr(C)]
struct Extents {
    count: u32,
    extents: [DISK_EXTENT; 16],
}

/// Disk number of the single physical disk backing `X:\`. Opens the volume with zero
/// access rights (allowed without admin), so no data can be read or written.
fn volume_disk_number(mount_point: &str) -> Option<u32> {
    let letter = crate::bench_env::normalize_drive_mount(mount_point)?;
    let device = format!("\\\\.\\{}", &letter[..2]);
    let file = OpenOptions::new()
        .access_mode(0)
        .share_mode(0x1 | 0x2) // FILE_SHARE_READ | FILE_SHARE_WRITE
        .open(&device)
        .ok()?;
    let mut out = Extents {
        count: 0,
        extents: [DISK_EXTENT::default(); 16],
    };
    let mut returned = 0u32;
    let size = u32::try_from(std::mem::size_of::<Extents>()).ok()?;
    // SAFETY: valid handle owned by `file` for the call; output buffer is `size` bytes.
    unsafe {
        DeviceIoControl(
            HANDLE(file.as_raw_handle()),
            IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
            None,
            0,
            Some((&raw mut out).cast()),
            size,
            Some(&mut returned),
            None,
        )
    }
    .ok()?;
    let n = (out.count as usize).min(out.extents.len());
    let first = out.extents.first()?.DiskNumber;
    (n >= 1 && out.extents[..n].iter().all(|e| e.DiskNumber == first)).then_some(first)
}

pub fn volume_disk_model(mount_point: &str) -> Option<String> {
    let disk = volume_disk_number(mount_point)?;
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("wmi-disk-model".into())
        .spawn(move || {
            let _ = tx.send(super::wmi::physical_disks_blocking());
        })
        .ok()?;
    let disks = rx.recv_timeout(WMI_TIMEOUT).ok()?;
    let id = disk.to_string();
    disks
        .value()?
        .iter()
        .find(|d| d.id == id)
        .and_then(|d| d.model.clone())
}
