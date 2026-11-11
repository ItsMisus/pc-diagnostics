//! Raw per-process counters for the process analyser (`crates/processes`).
//!
//! On Windows one `NtQuerySystemInformation(SystemProcessInformation)` call per tick
//! returns CPU time, memory, I/O transfer counts, creation time, parent and session for
//! **every** process, without opening a handle to any of them. That is what Task Manager
//! and the PDH `Process` object use, it works without admin rights, and it costs a few
//! milliseconds for hundreds of processes.
//!
//! Handles are opened only for the executable path (`PROCESS_QUERY_LIMITED_INFORMATION`,
//! read-only), once per process instance; the caller caches the answer. Nothing here can
//! modify, suspend or signal a process.

use std::collections::HashSet;
use std::time::Instant;

use models::AppError;

/// 100 ns intervals between 1601-01-01 and 1970-01-01 (FILETIME epoch offset).
pub const FILETIME_UNIX_EPOCH: i64 = 116_444_736_000_000_000;

/// Counters of one process as reported by the OS at scan time. Times are cumulative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawProcess {
    pub pid: u32,
    pub parent_pid: u32,
    /// Image name (e.g. `explorer.exe`). Empty for the System Idle Process.
    pub name: String,
    pub session_id: u32,
    /// Creation time, FILETIME units (100 ns since 1601). 0 for boot-time pseudo processes.
    pub create_time_100ns: i64,
    /// Kernel + user CPU time, 100 ns units, summed across all threads.
    pub cpu_time_100ns: u64,
    pub working_set_bytes: u64,
    /// Private committed bytes (Task Manager "Commit size").
    pub private_bytes: u64,
    /// Cumulative logical read / write transfer counts (files, network, devices).
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

/// One enumeration of all processes.
#[derive(Debug, Clone)]
pub struct ProcessScan {
    /// Wall clock at scan time, Unix ms.
    pub at_unix_ms: u64,
    /// Monotonic clock at scan time (use for rate windows).
    pub taken: Instant,
    pub processes: Vec<RawProcess>,
}

/// Result of trying to read a process's executable path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageAccess {
    /// The process could be opened. `path` is `None` if the path query itself failed
    /// (e.g. kernel pseudo-processes such as `Registry` have no Win32 image path).
    Accessible { path: Option<String> },
    /// `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` was denied.
    Denied,
    /// The pid is gone or now belongs to a different process instance.
    Exited,
}

/// Physical memory totals (`GlobalMemoryStatusEx`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStatus {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// FILETIME (100 ns since 1601) to Unix ms. Times before 1970 (or 0) map to 0.
pub fn filetime_to_unix_ms(ft_100ns: i64) -> u64 {
    u64::try_from((ft_100ns - FILETIME_UNIX_EPOCH) / 10_000).unwrap_or(0)
}

/// Unix ms to FILETIME units (inverse of [`filetime_to_unix_ms`] at ms precision).
pub fn unix_ms_to_filetime(ms: u64) -> i64 {
    i64::try_from(ms)
        .ok()
        .and_then(|m| m.checked_mul(10_000))
        .and_then(|v| v.checked_add(FILETIME_UNIX_EPOCH))
        .unwrap_or(i64::MAX)
}

/// Enumerates all processes. Keeps its query buffer between calls (no per-tick
/// reallocation once it has grown to fit). Lives on one thread.
pub struct ProcessEnumerator {
    #[cfg(windows)]
    inner: crate::windows::process::Enumerator,
    #[cfg(target_os = "macos")]
    inner: crate::macos::process::Enumerator,
}

impl ProcessEnumerator {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            inner: crate::windows::process::Enumerator::new(),
            #[cfg(target_os = "macos")]
            inner: crate::macos::process::Enumerator::new(),
        }
    }

    pub fn scan(&mut self) -> Result<ProcessScan, AppError> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            self.inner.scan()
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            Err(not_supported())
        }
    }
}

impl Default for ProcessEnumerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Opens `pid` with `PROCESS_QUERY_LIMITED_INFORMATION` (read-only) and reads its full
/// Win32 image path. `create_time_100ns` guards against PID reuse: if the opened process
/// has a different creation time, the answer is `Exited`.
pub fn image_access(pid: u32, create_time_100ns: i64) -> ImageAccess {
    #[cfg(windows)]
    {
        crate::windows::process::image_access(pid, create_time_100ns)
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::process::image_access(pid, create_time_100ns)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (pid, create_time_100ns);
        ImageAccess::Denied
    }
}

/// Pids owning at least one visible, non-cloaked top-level window (one `EnumWindows`).
pub fn visible_window_pids() -> Result<HashSet<u32>, AppError> {
    #[cfg(windows)]
    {
        crate::windows::process::visible_window_pids()
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::process::visible_window_pids()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err(not_supported())
    }
}

pub fn memory_status() -> Result<MemoryStatus, AppError> {
    #[cfg(windows)]
    {
        crate::windows::process::memory_status()
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::process::memory_status()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Err(not_supported())
    }
}

/// `%SystemRoot%\System32` (from `GetSystemDirectoryW`), used to validate core OS images.
pub fn system_directory() -> Option<String> {
    #[cfg(windows)]
    {
        crate::windows::process::system_directory()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn not_supported() -> AppError {
    AppError::new(
        models::ErrorCode::NotSupported,
        "process enumeration is implemented for Windows and macOS only",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_conversion() {
        assert_eq!(filetime_to_unix_ms(FILETIME_UNIX_EPOCH), 0);
        assert_eq!(filetime_to_unix_ms(0), 0, "pre-1970 clamps");
        assert_eq!(filetime_to_unix_ms(FILETIME_UNIX_EPOCH + 12_345_678), 1_234);
        let ms = 1_762_000_000_123;
        assert_eq!(filetime_to_unix_ms(unix_ms_to_filetime(ms)), ms);
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn host_scan_lists_self_and_counters_grow() {
        let mut e = ProcessEnumerator::new();
        let a = e.scan().expect("scan");
        assert!(a.processes.len() > 10, "{}", a.processes.len());
        let me = std::process::id();
        let mine = a
            .processes
            .iter()
            .find(|p| p.pid == me)
            .expect("own pid present")
            .clone();
        assert!(mine.working_set_bytes > 0);
        assert!(mine.private_bytes > 0);
        assert!(mine.create_time_100ns > FILETIME_UNIX_EPOCH);
        #[cfg(windows)]
        {
            assert!(mine.name.to_ascii_lowercase().ends_with(".exe"));
            assert!(a.processes.iter().any(|p| p.pid == 4), "System present");
        }
        #[cfg(target_os = "macos")]
        assert!(a.processes.iter().any(|p| p.pid == 1), "launchd present");
        // Burn CPU, then the counter must have advanced.
        let t = Instant::now();
        let mut x = 0u64;
        while t.elapsed().as_millis() < 100 {
            x = x.wrapping_mul(31).wrapping_add(7);
        }
        std::hint::black_box(x);
        let b = e.scan().expect("scan");
        let again = b.processes.iter().find(|p| p.pid == me).expect("own pid");
        assert!(again.cpu_time_100ns > mine.cpu_time_100ns);
        assert_eq!(again.create_time_100ns, mine.create_time_100ns);
        match image_access(me, mine.create_time_100ns) {
            ImageAccess::Accessible { path: Some(p) } => {
                #[cfg(windows)]
                assert!(p.to_ascii_lowercase().ends_with(".exe"), "{p}");
                #[cfg(not(windows))]
                assert!(p.starts_with('/'), "{p}");
            }
            other => panic!("own process must be accessible: {other:?}"),
        }
        assert_eq!(
            image_access(me, mine.create_time_100ns + 1),
            ImageAccess::Exited,
            "creation-time mismatch means a different instance"
        );
        let m = memory_status().expect("memory");
        assert!(m.total_bytes > 0 && m.available_bytes <= m.total_bytes);
        #[cfg(windows)]
        assert!(system_directory().is_some());
        let _ = visible_window_pids().expect("enum windows");
    }
}
