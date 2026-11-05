//! Process enumeration: `NtQuerySystemInformation(SystemProcessInformation)`, plus
//! read-only `PROCESS_QUERY_LIMITED_INFORMATION` handles for executable paths,
//! `EnumWindows` for window ownership and `GlobalMemoryStatusEx`.
//!
//! `SYSTEM_PROCESS_INFORMATION` in the Windows SDK hides several fields behind
//! `Reserved*` members. Their layout is the long-standing one used by Task Manager,
//! perfproc and every process tool since Windows Vista (see phnt `ntexapi.h`):
//!
//! - `Reserved1` (48 bytes): `WorkingSetPrivateSize: i64`, `HardFaultCount: u32`,
//!   `NumberOfThreadsHighWatermark: u32`, `CycleTime: u64`, `CreateTime: i64`,
//!   `UserTime: i64`, `KernelTime: i64`.
//! - `Reserved2`: `InheritedFromUniqueProcessId` (parent pid).
//! - `Reserved7` (6 × i64): `Read/Write/OtherOperationCount`,
//!   `Read/Write/OtherTransferCount`.

use std::collections::HashSet;
use std::mem::size_of;
use std::time::Instant;

use ::windows::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SystemProcessInformation,
};
use ::windows::Win32::Foundation::{
    CloseHandle, E_ACCESSDENIED, ERROR_INSUFFICIENT_BUFFER, ERROR_INVALID_PARAMETER, FILETIME,
    HANDLE, HWND, LPARAM, STATUS_BUFFER_TOO_SMALL, STATUS_INFO_LENGTH_MISMATCH,
};
use ::windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use ::windows::Win32::System::SystemInformation::{
    GetSystemDirectoryW, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use ::windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use ::windows::Win32::System::WindowsProgramming::SYSTEM_PROCESS_INFORMATION;
use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
};
use ::windows::core::{BOOL, HRESULT, PWSTR};
use models::{AppError, ErrorCode};

use crate::process::{ImageAccess, MemoryStatus, ProcessScan, RawProcess};

// The decoding below relies on the x64 / arm64 layout of the SDK struct.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(size_of::<SYSTEM_PROCESS_INFORMATION>() == 0x100);
    assert!(std::mem::offset_of!(SYSTEM_PROCESS_INFORMATION, ImageName) == 0x38);
    assert!(std::mem::offset_of!(SYSTEM_PROCESS_INFORMATION, Reserved7) == 0xD0);
};

/// Initial query buffer (grows on demand and is then reused).
const INITIAL_BUFFER_BYTES: usize = 512 * 1024;
/// Hard cap so a misbehaving call can never make us allocate without bound.
const MAX_BUFFER_BYTES: usize = 64 * 1024 * 1024;
/// Extra room added on growth: processes and threads appear between calls.
const GROWTH_SLACK_BYTES: usize = 64 * 1024;

pub struct Enumerator {
    /// `u64` elements keep the buffer 8-byte aligned, as the entries require.
    buf: Vec<u64>,
}

impl Enumerator {
    pub fn new() -> Self {
        Self {
            buf: vec![0; INITIAL_BUFFER_BYTES / 8],
        }
    }

    pub fn scan(&mut self) -> Result<ProcessScan, AppError> {
        let valid = self.query()?;
        let taken = Instant::now();
        let at_unix_ms = models::time::now_unix_ms();
        let processes = parse(&self.buf, valid);
        Ok(ProcessScan {
            at_unix_ms,
            taken,
            processes,
        })
    }

    /// Fills the buffer; returns the number of valid bytes.
    fn query(&mut self) -> Result<usize, AppError> {
        for _ in 0..8 {
            let len_bytes = self.buf.len() * 8;
            let len = u32::try_from(len_bytes).unwrap_or(u32::MAX);
            let mut needed = 0u32;
            // SAFETY: the buffer is `len` bytes long, writable and 8-byte aligned;
            // `needed` is a valid out pointer. The call only writes inside the buffer.
            let status = unsafe {
                NtQuerySystemInformation(
                    SystemProcessInformation,
                    self.buf.as_mut_ptr().cast(),
                    len,
                    &mut needed,
                )
            };
            if status == STATUS_INFO_LENGTH_MISMATCH || status == STATUS_BUFFER_TOO_SMALL {
                let want = (needed as usize).max(len_bytes) + GROWTH_SLACK_BYTES;
                let want = if want <= len_bytes {
                    len_bytes * 2
                } else {
                    want
                };
                if want > MAX_BUFFER_BYTES {
                    break;
                }
                self.buf = vec![0; want.div_ceil(8)];
                continue;
            }
            if status.is_err() {
                return Err(AppError::new(
                    ErrorCode::Platform,
                    "NtQuerySystemInformation(SystemProcessInformation) failed",
                )
                .with_detail(format!("NTSTATUS 0x{:08X}", status.0)));
            }
            return Ok((needed as usize).min(len_bytes));
        }
        Err(AppError::new(
            ErrorCode::Platform,
            "process information buffer could not be sized",
        ))
    }
}

fn i64_at(bytes: &[u8; 48], offset: usize) -> i64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes[offset..offset + 8]);
    i64::from_le_bytes(b)
}

/// Walks the `NextEntryOffset` chain. Every entry and every name is bounds-checked
/// against the `valid` bytes of `buf`; a malformed chain ends the walk.
fn parse(buf: &[u64], valid: usize) -> Vec<RawProcess> {
    let base = buf.as_ptr().cast::<u8>();
    let valid = valid.min(buf.len() * 8);
    let entry_size = size_of::<SYSTEM_PROCESS_INFORMATION>();
    let mut out = Vec::with_capacity(512);
    let mut offset = 0usize;
    while offset
        .checked_add(entry_size)
        .is_some_and(|end| end <= valid)
    {
        // SAFETY: `offset + entry_size <= valid <= buf` bytes, so the read is in bounds;
        // `read_unaligned` imposes no alignment requirement; the struct is plain data.
        let e: SYSTEM_PROCESS_INFORMATION =
            unsafe { std::ptr::read_unaligned(base.add(offset).cast()) };
        let name = image_name(&e, base, valid);
        let create = i64_at(&e.Reserved1, 24);
        let user = i64_at(&e.Reserved1, 32);
        let kernel = i64_at(&e.Reserved1, 40);
        let nonneg = |v: i64| u64::try_from(v).unwrap_or(0);
        out.push(RawProcess {
            pid: u32::try_from(e.UniqueProcessId.0 as usize).unwrap_or(u32::MAX),
            parent_pid: u32::try_from(e.Reserved2 as usize).unwrap_or(0),
            name,
            session_id: e.SessionId,
            create_time_100ns: create.max(0),
            cpu_time_100ns: nonneg(user).saturating_add(nonneg(kernel)),
            working_set_bytes: e.WorkingSetSize as u64,
            private_bytes: e.PrivatePageCount as u64,
            io_read_bytes: nonneg(e.Reserved7[3]),
            io_write_bytes: nonneg(e.Reserved7[4]),
        });
        if e.NextEntryOffset == 0 {
            break;
        }
        offset += e.NextEntryOffset as usize;
    }
    out
}

/// The name buffer points into the query buffer; anything else is rejected.
fn image_name(e: &SYSTEM_PROCESS_INFORMATION, base: *const u8, valid: usize) -> String {
    let ptr = e.ImageName.Buffer.0 as usize;
    let len = usize::from(e.ImageName.Length);
    let start = base as usize;
    if ptr == 0 || len == 0 || !len.is_multiple_of(2) || !ptr.is_multiple_of(2) {
        return String::new();
    }
    if ptr < start || ptr.saturating_add(len) > start + valid {
        return String::new();
    }
    // SAFETY: the range [ptr, ptr+len) was just checked to lie inside the initialized
    // part of our buffer, and `ptr` is 2-byte aligned as `u16` requires.
    let units = unsafe { std::slice::from_raw_parts(ptr as *const u16, len / 2) };
    String::from_utf16_lossy(units)
}

pub(crate) struct OwnedHandle(pub(crate) HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenProcess and is closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(crate) fn filetime_100ns(ft: FILETIME) -> i64 {
    i64::try_from((u64::from(ft.dwHighDateTime) << 32) | u64::from(ft.dwLowDateTime))
        .unwrap_or(i64::MAX)
}

pub fn image_access(pid: u32, create_time_100ns: i64) -> ImageAccess {
    // SAFETY: plain call; the handle is owned (and closed) by `OwnedHandle`.
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => OwnedHandle(h),
        Err(e) if e.code() == E_ACCESSDENIED => return ImageAccess::Denied,
        Err(e) if e.code() == HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) => {
            return ImageAccess::Exited;
        }
        Err(e) => {
            tracing::debug!(pid, error = %e, "OpenProcess failed");
            return ImageAccess::Denied;
        }
    };
    let (mut c, mut x, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: valid handle with PROCESS_QUERY_LIMITED_INFORMATION; four valid out pointers.
    if unsafe { GetProcessTimes(handle.0, &mut c, &mut x, &mut k, &mut u) }.is_ok()
        && filetime_100ns(c) != create_time_100ns
        && create_time_100ns != 0
    {
        return ImageAccess::Exited;
    }
    ImageAccess::Accessible {
        path: query_image_path(&handle),
    }
}

fn query_image_path(handle: &OwnedHandle) -> Option<String> {
    for cap in [1_024usize, 32_768] {
        let mut buf = vec![0u16; cap];
        let mut len = u32::try_from(cap).unwrap_or(u32::MAX);
        // SAFETY: `buf` holds `len` UTF-16 units; `len` is updated to the written length.
        let r = unsafe {
            QueryFullProcessImageNameW(
                handle.0,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
        };
        match r {
            Ok(()) => {
                let n = (len as usize).min(cap);
                return Some(String::from_utf16_lossy(&buf[..n])).filter(|s| !s.is_empty());
            }
            Err(e) if e.code() == HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0) => {}
            Err(_) => return None,
        }
    }
    None
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the `&mut HashSet<u32>` passed by `visible_window_pids`, which
    // outlives the synchronous `EnumWindows` call and is not aliased during it.
    let set = unsafe { &mut *(lparam.0 as *mut HashSet<u32>) };
    // SAFETY: `hwnd` is a live top-level window handed to us by EnumWindows.
    if unsafe { IsWindowVisible(hwnd) }.as_bool() && !is_cloaked(hwnd) {
        let mut pid = 0u32;
        // SAFETY: valid out pointer.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid != 0 {
            set.insert(pid);
        }
    }
    BOOL(1)
}

/// Cloaked windows (suspended UWP apps, windows on other virtual desktops' shells, …)
/// are "visible" to `IsWindowVisible` but not shown to the user.
pub(crate) fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    // SAFETY: the out buffer is a u32 of the declared size, as DWMWA_CLOAKED requires.
    let r = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            u32::try_from(size_of::<u32>()).unwrap_or(4),
        )
    };
    r.is_ok() && cloaked != 0
}

pub fn visible_window_pids() -> Result<HashSet<u32>, AppError> {
    let mut set: HashSet<u32> = HashSet::with_capacity(64);
    // SAFETY: the callback only dereferences `lparam` as the set, which lives across the
    // call; EnumWindows is synchronous.
    unsafe { EnumWindows(Some(collect_window), LPARAM((&raw mut set) as isize)) }.map_err(|e| {
        AppError::new(ErrorCode::Platform, "EnumWindows failed").with_detail(e.to_string())
    })?;
    Ok(set)
}

pub fn memory_status() -> Result<MemoryStatus, AppError> {
    let mut m = MEMORYSTATUSEX {
        dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: `m` is a valid MEMORYSTATUSEX with `dwLength` set as required.
    unsafe { GlobalMemoryStatusEx(&mut m) }.map_err(|e| {
        AppError::new(ErrorCode::Platform, "GlobalMemoryStatusEx failed").with_detail(e.to_string())
    })?;
    Ok(MemoryStatus {
        total_bytes: m.ullTotalPhys,
        available_bytes: m.ullAvailPhys.min(m.ullTotalPhys),
    })
}

pub fn system_directory() -> Option<String> {
    let mut buf = vec![0u16; 1_024];
    // SAFETY: the buffer slice carries its own length.
    let n = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
    (n > 0 && n < buf.len()).then(|| String::from_utf16_lossy(&buf[..n]))
}
