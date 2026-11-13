//! Process enumeration on macOS through `sysinfo` (`proc_listallpids` + `proc_pidinfo`),
//! with no admin rights and no handles kept open.
//!
//! Field mapping onto the Windows-shaped [`RawProcess`]:
//! - `cpu_time_100ns`: user + system CPU time (sysinfo reports ms), converted to 100 ns.
//! - `working_set_bytes`: resident size.
//! - `private_bytes`: the physical footprint (`proc_pid_rusage` `ri_phys_footprint`), the
//!   value Activity Monitor shows in its "Memory" column: memory charged to the process
//!   (private dirty, compressed and swapped pages, IOKit/GPU allocations it owns), not
//!   shared library pages. macOS has no per-process commit charge, so this is the closest
//!   meaning to Windows' private bytes. It is readable without privileges only for the
//!   user's own processes (about 2.5 µs per call on an M5); for other users' processes
//!   the call fails with EPERM and the value is reported as unavailable.
//! - `io_*_bytes`: cumulative disk bytes (`rusage` `ri_diskio_*`). macOS does not count
//!   network or device I/O per process, unlike the Windows transfer counters.
//! - `session_id`: there are no Windows sessions. Processes of system accounts (uid
//!   below 500, e.g. root and `_` daemon users) map to session 0, which the categorizer
//!   treats as services; user accounts map to 1; an unreadable owner to `u32::MAX`.
//! - `create_time_100ns`: start time (whole seconds) in FILETIME units, so PID-reuse
//!   checks keep working.
//!
//! Processes owned by other users (root daemons) can be listed but their counters may
//! be unreadable without privileges; sysinfo then reports zeros for them.

use std::collections::HashSet;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use models::{AppError, ErrorCode};
use sysinfo::{MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::process::{ImageAccess, MemoryStatus, ProcessScan, RawProcess, unix_ms_to_filetime};

/// First uid of a regular (human) account on macOS.
const FIRST_USER_UID: u32 = 500;

fn new_system() -> Result<System, AppError> {
    System::new().map_err(|e| {
        AppError::new(ErrorCode::Platform, "sysinfo initialization failed")
            .with_detail(e.to_string())
    })
}

/// Keeps one `System` between scans (sysinfo needs the previous values for its own
/// bookkeeping and reuses its allocations).
pub struct Enumerator {
    sys: Option<System>,
}

impl Enumerator {
    pub fn new() -> Self {
        Self { sys: None }
    }

    pub fn scan(&mut self) -> Result<ProcessScan, AppError> {
        let sys = match &mut self.sys {
            Some(sys) => sys,
            empty => empty.insert(new_system()?),
        };
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage()
                .with_user(UpdateKind::OnlyIfNotSet),
        );
        let taken = Instant::now();
        let at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        let processes = sys
            .processes()
            .iter()
            .map(|(pid, p)| {
                let disk = p.disk_usage();
                let session_id = match p.user_id() {
                    Some(uid) if **uid < FIRST_USER_UID => 0,
                    Some(_) => 1,
                    None => u32::MAX,
                };
                RawProcess {
                    pid: pid.as_u32(),
                    parent_pid: p.parent().map_or(0, Pid::as_u32),
                    name: p.name().to_string_lossy().into_owned(),
                    session_id,
                    create_time_100ns: start_time_100ns(p.start_time()),
                    cpu_time_100ns: p.accumulated_cpu_time().saturating_mul(10_000),
                    working_set_bytes: p.memory(),
                    private_bytes: phys_footprint(pid.as_u32()),
                    io_read_bytes: disk.total_read_bytes,
                    io_write_bytes: disk.total_written_bytes,
                }
            })
            .collect();
        Ok(ProcessScan {
            at_unix_ms,
            taken,
            processes,
        })
    }
}

/// Physical footprint of `pid` (`ri_phys_footprint`). `None` when the OS refuses
/// (EPERM for other users' processes) or the process is gone (ESRCH).
pub fn phys_footprint(pid: u32) -> Option<u64> {
    let pid = libc::c_int::try_from(pid).ok()?;
    // SAFETY: `rusage_info_v4` is plain integers, so all-zero is a valid value.
    let mut info: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
    // SAFETY: `info` is a writable `rusage_info_v4`, exactly the buffer the
    // `RUSAGE_INFO_V4` flavor fills; the pointer is only used for this call.
    let r = unsafe {
        libc::proc_pid_rusage(
            pid,
            libc::RUSAGE_INFO_V4,
            (&raw mut info).cast::<libc::rusage_info_t>(),
        )
    };
    (r == 0).then_some(info.ri_phys_footprint)
}

fn start_time_100ns(start_secs: u64) -> i64 {
    if start_secs == 0 {
        return 0;
    }
    unix_ms_to_filetime(start_secs.saturating_mul(1_000))
}

/// Reads the executable path of `pid`. `create_time_100ns` guards against PID reuse.
pub fn image_access(pid: u32, create_time_100ns: i64) -> ImageAccess {
    let Ok(mut sys) = new_system() else {
        return ImageAccess::Denied;
    };
    let target = Pid::from_u32(pid);
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[target]),
        false,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    let Some(p) = sys.process(target) else {
        return ImageAccess::Exited;
    };
    if create_time_100ns != 0 && start_time_100ns(p.start_time()) != create_time_100ns {
        return ImageAccess::Exited;
    }
    match p.exe() {
        Some(path) => ImageAccess::Accessible {
            path: Some(path.to_string_lossy().into_owned()),
        },
        // proc_pidpath fails for processes of other users without privileges.
        None => ImageAccess::Denied,
    }
}

/// Pids that own an app bundle executable (`*.app/Contents/MacOS/*`) in this scan.
///
/// macOS lists on-screen windows only through CoreGraphics; until that provider exists,
/// an app bundle executable started by the user is the closest signal for "has a
/// window". It is used only to split user processes into applications and background.
pub fn visible_window_pids() -> Result<HashSet<u32>, AppError> {
    let mut sys = new_system()?;
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_user(UpdateKind::OnlyIfNotSet),
    );
    Ok(sys
        .processes()
        .iter()
        .filter(|(_, p)| p.user_id().is_some_and(|uid| **uid >= FIRST_USER_UID))
        .filter(|(_, p)| {
            p.exe()
                .and_then(|e| e.to_str())
                .is_some_and(is_app_bundle_main_executable)
        })
        .map(|(pid, _)| pid.as_u32())
        .collect())
}

/// `/…/Name.app/Contents/MacOS/Name`, excluding helpers nested in another bundle
/// (`…/Frameworks/…` or `…/Helpers/…`, e.g. browser renderer processes).
fn is_app_bundle_main_executable(path: &str) -> bool {
    let Some(idx) = path.rfind(".app/Contents/MacOS/") else {
        return false;
    };
    let bundle = &path[..idx];
    let rest = &path[idx + ".app/Contents/MacOS/".len()..];
    !rest.contains('/') && !bundle.contains("/Frameworks/") && !bundle.contains("/Helpers/")
}

pub fn memory_status() -> Result<MemoryStatus, AppError> {
    let mut sys = new_system()?;
    sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    let total = sys.total_memory();
    Ok(MemoryStatus {
        total_bytes: total,
        available_bytes: sys.available_memory().min(total),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_phys_footprint_own_process_only() {
        let me = phys_footprint(std::process::id()).expect("own process footprint");
        // A running test binary owns at least a few hundred KiB and far less than RAM.
        assert!(me > 256 * 1024 && me < 64 * 1024 * 1024 * 1024, "{me}");
        // launchd (pid 1) belongs to root: refused without privileges.
        assert_eq!(phys_footprint(1), None);
        assert_eq!(phys_footprint(u32::MAX), None);
    }

    #[test]
    fn app_bundle_detection() {
        assert!(is_app_bundle_main_executable(
            "/Applications/Safari.app/Contents/MacOS/Safari"
        ));
        assert!(!is_app_bundle_main_executable(
            "/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Helpers/Google Chrome Helper (Renderer).app/Contents/MacOS/Google Chrome Helper (Renderer)"
        ));
        assert!(!is_app_bundle_main_executable("/usr/sbin/cfprefsd"));
        assert!(!is_app_bundle_main_executable(
            "/Applications/X.app/Contents/MacOS/sub/tool"
        ));
    }
}
