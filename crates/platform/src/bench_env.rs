//! Facts the benchmark engine needs about the run environment (phase 5): power source,
//! active power plan, CPU frequency limit, fixed volumes, free space, sector size and the
//! physical disk behind a volume.
//!
//! Everything here is read-only. Nothing changes power, BIOS or registry settings.
//! Values that cannot be determined are `None`/`Unavailable`; nothing is guessed.
//!
//! Volumes are named by their mount point: `C:\` on Windows, `/` or `/Volumes/<name>` on
//! macOS (see `macos::bench_env` for how `/` stands for the system volume group). The only
//! write-side helpers are the macOS file controls the storage benchmark applies to its own
//! temporary file ([`disable_cache`], [`full_fsync`]).

#[cfg(target_os = "macos")]
use std::fs::File;

use models::AppError;
use models::benchmark_run::PowerSource;
use models::metric::Metric;
#[cfg(not(windows))]
use models::metric::Unavailability;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

use crate::convert::non_empty;

/// Free and total space of a volume, as seen by the current user (quotas applied).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeSpace {
    pub free_to_caller_bytes: u64,
    pub total_bytes: u64,
}

/// AC line status from `GetSystemPowerStatus` (Windows), or the providing power source
/// from IOPowerSources (macOS).
pub fn power_source() -> PowerSource {
    #[cfg(windows)]
    {
        crate::windows::bench_env::power_source()
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::bench_env::power_source()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        PowerSource::Unknown
    }
}

/// Friendly name of the active power plan (`PowerGetActiveScheme` +
/// `PowerReadFriendlyName`), e.g. "Bilanciato". macOS has no power plans: there it is the
/// Low Power Mode state, "Low Power Mode: on" or "Low Power Mode: off".
pub fn active_power_plan() -> Option<String> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::active_power_plan()
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::bench_env::active_power_plan()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// CPU brand string (first logical processor).
pub fn cpu_brand() -> Option<String> {
    let sys =
        System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing()))
            .ok()?;
    non_empty(sys.cpus().first().map(|c| c.brand().trim().to_owned()))
}

/// Installed physical memory in bytes (0 if unknown).
pub fn total_memory_bytes() -> u64 {
    System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    )
    .map_or(0, |s| s.total_memory())
}

/// `(max_mhz, limit_mhz)` averaged across logical processors, read directly from
/// `CallNtPowerInformation` (same method as the live sampler). Cheap.
///
/// macOS: `NotSupported` for both. Apple Silicon exposes neither a maximum nor an OS
/// frequency limit to unprivileged processes (no `hw.cpufrequency*` sysctls on arm64; the
/// DVFS state is only visible to `powermetrics`, which needs root).
pub fn cpu_frequency_limits() -> (Metric<f64>, Metric<f64>) {
    #[cfg(windows)]
    {
        crate::windows::bench_env::cpu_frequency_limits()
    }
    #[cfg(not(windows))]
    {
        (
            Metric::unavailable(Unavailability::NotSupported),
            Metric::unavailable(Unavailability::NotSupported),
        )
    }
}

/// Mount points (`C:\`) of the present fixed volumes (`GetDriveTypeW == DRIVE_FIXED`).
/// macOS: `/` first, then the local, writable, non-removable volumes (exact rule in
/// `macos::bench_env::fixed_volumes`).
pub fn fixed_volumes() -> Vec<String> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::fixed_volumes()
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::bench_env::fixed_volumes()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

/// Mount point of the volume that holds Windows (`C:\`), from `GetSystemWindowsDirectoryW`.
/// macOS: `/` (the booted system volume group).
pub fn system_volume() -> Option<String> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::system_volume()
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::bench_env::system_volume()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// Free/total space of the volume containing `path` (`GetDiskFreeSpaceExW`; `statfs` on
/// macOS, where APFS volumes report the space of their shared container).
pub fn volume_space(path: &str) -> Result<VolumeSpace, AppError> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::volume_space(path)
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::bench_env::volume_space(path)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = path;
        Err(AppError::new(
            models::ErrorCode::NotSupported,
            "volume space is not implemented on this OS",
        ))
    }
}

/// Logical bytes per sector of a volume root (`GetDiskFreeSpaceW`). macOS: the I/O Kit
/// `IOMedia` "Preferred Block Size" of the device mounted there.
pub fn volume_sector_size(mount_point: &str) -> Option<u32> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::volume_sector_size(mount_point)
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::bench_env::volume_sector_size(mount_point)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = mount_point;
        None
    }
}

/// Model (`MSFT_PhysicalDisk.FriendlyName`) of the single physical disk backing a volume.
/// `None` when the volume spans several disks, the disk cannot be resolved, or WMI fails.
/// Slow (WMI): call it off latency-sensitive threads. macOS: the "Product Name" of the
/// block storage device under the volume (I/O Kit registry).
pub fn volume_disk_model(mount_point: &str) -> Option<String> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::volume_disk_model(mount_point)
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::bench_env::volume_disk_model(mount_point)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = mount_point;
        None
    }
}

/// macOS: the fixed volume (as listed by [`fixed_volumes`]) that contains `path`. A path on
/// the data volume of the booted volume group (e.g. `$TMPDIR`) maps to `/`.
#[cfg(target_os = "macos")]
pub fn volume_of(path: &str) -> Option<String> {
    crate::macos::bench_env::volume_of(path)
}

/// macOS: the per-user temporary directory assigned by the OS (independent of `$TMPDIR`).
#[cfg(target_os = "macos")]
pub fn user_temp_dir() -> Option<std::path::PathBuf> {
    crate::macos::bench_env::user_temp_dir()
}

/// macOS: bypass the unified buffer cache for `file` (`F_NOCACHE`). Storage benchmark only.
#[cfg(target_os = "macos")]
pub fn disable_cache(file: &File) -> Result<(), AppError> {
    crate::macos::bench_env::disable_cache(file)
}

/// macOS: flush `file` to the device including the drive's own cache (`F_FULLFSYNC`).
#[cfg(target_os = "macos")]
pub fn full_fsync(file: &File) -> Result<(), AppError> {
    crate::macos::bench_env::full_fsync(file)
}

/// Normalizes a user-supplied volume mount point.
///
/// Windows (and other non-macOS targets): to `X:\` (upper case). Accepts `c`, `c:`, `C:\`,
/// `C:/`. Anything else (UNC, paths, devices) is rejected.
///
/// macOS: an absolute mount path such as `/` or `/Volumes/Data SSD` (trailing slashes
/// removed; `.`, `..` and empty components rejected). Whether it names a fixed volume is
/// checked by the caller against [`fixed_volumes`].
pub fn normalize_drive_mount(input: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        crate::macos::bench_env::normalize_mount_point(input)
    }
    #[cfg(not(target_os = "macos"))]
    {
        normalize_drive_letter(input)
    }
}

#[cfg(not(target_os = "macos"))]
fn normalize_drive_letter(input: &str) -> Option<String> {
    let s = input.trim();
    let mut chars = s.chars();
    let letter = chars.next()?;
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    let rest: String = chars.collect();
    if matches!(rest.as_str(), "" | ":" | ":\\" | ":/") {
        Some(format!("{}:\\", letter.to_ascii_uppercase()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn drive_mounts_normalize() {
        assert_eq!(normalize_drive_mount("c").as_deref(), Some("C:\\"));
        assert_eq!(normalize_drive_mount("C:").as_deref(), Some("C:\\"));
        assert_eq!(normalize_drive_mount(" d:\\ ").as_deref(), Some("D:\\"));
        assert_eq!(normalize_drive_mount("e:/").as_deref(), Some("E:\\"));
        for bad in [
            "",
            "1:",
            "C:\\Windows",
            "\\\\server\\share",
            "\\\\.\\C:",
            "CC:",
        ] {
            assert_eq!(normalize_drive_mount(bad), None, "{bad}");
        }
    }

    // Host smoke test: invariants only.
    #[test]
    fn host_environment_is_plausible() {
        assert!(total_memory_bytes() > 0);
        assert!(cpu_brand().is_some());
        if cfg!(windows) {
            let sys = system_volume().expect("system volume");
            let vols = fixed_volumes();
            assert!(vols.contains(&sys), "{sys} in {vols:?}");
            let space = volume_space(&sys).expect("space");
            assert!(space.free_to_caller_bytes <= space.total_bytes);
            let sector = volume_sector_size(&sys).expect("sector size");
            assert!(sector.is_power_of_two() && sector >= 512);
            let (max, limit) = cpu_frequency_limits();
            assert!(max.value().is_some_and(|v| *v > 0.0));
            assert!(limit.value().is_some_and(|v| *v > 0.0));
            assert_ne!(power_source(), PowerSource::Unknown);
            assert!(active_power_plan().is_some());
        }
        #[cfg(target_os = "macos")]
        {
            let sys = system_volume().expect("system volume");
            assert_eq!(sys, "/");
            let vols = fixed_volumes();
            assert!(vols.contains(&sys), "{sys} in {vols:?}");
            let space = volume_space(&sys).expect("space");
            assert!(space.free_to_caller_bytes <= space.total_bytes);
            let sector = volume_sector_size(&sys).expect("block size");
            assert!(sector.is_power_of_two() && sector >= 512);
            // Apple Silicon: no readable frequency limits (explained, never zero).
            let (max, limit) = cpu_frequency_limits();
            assert_eq!(max, Metric::unavailable(Unavailability::NotSupported));
            assert_eq!(limit, Metric::unavailable(Unavailability::NotSupported));
            assert_ne!(power_source(), PowerSource::Unknown);
            let plan = active_power_plan();
            assert!(
                plan.as_deref()
                    .is_some_and(|p| p.starts_with("Low Power Mode: ")),
                "{plan:?}"
            );
            assert_eq!(
                normalize_drive_mount("/Volumes/X/").as_deref(),
                Some("/Volumes/X")
            );
            assert_eq!(normalize_drive_mount("C:\\"), None);
            let tmp = std::env::temp_dir();
            assert_eq!(
                volume_of(tmp.to_str().expect("utf-8")).as_deref(),
                Some("/")
            );
        }
    }
}
