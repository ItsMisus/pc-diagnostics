//! Facts the benchmark engine needs about the run environment (phase 5): power source,
//! active power plan, CPU frequency limit, fixed volumes, free space, sector size and the
//! physical disk behind a volume.
//!
//! Everything here is read-only. Nothing changes power, BIOS or registry settings.
//! Values that cannot be determined are `None`/`Unavailable`; nothing is guessed.

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

/// AC line status from `GetSystemPowerStatus`.
pub fn power_source() -> PowerSource {
    #[cfg(windows)]
    {
        crate::windows::bench_env::power_source()
    }
    #[cfg(not(windows))]
    {
        PowerSource::Unknown
    }
}

/// Friendly name of the active power plan (`PowerGetActiveScheme` +
/// `PowerReadFriendlyName`), e.g. "Bilanciato".
pub fn active_power_plan() -> Option<String> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::active_power_plan()
    }
    #[cfg(not(windows))]
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
pub fn fixed_volumes() -> Vec<String> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::fixed_volumes()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Mount point of the volume that holds Windows (`C:\`), from `GetSystemWindowsDirectoryW`.
pub fn system_volume() -> Option<String> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::system_volume()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Free/total space of the volume containing `path` (`GetDiskFreeSpaceExW`).
pub fn volume_space(path: &str) -> Result<VolumeSpace, AppError> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::volume_space(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(AppError::new(
            models::ErrorCode::NotSupported,
            "volume space is Windows-only",
        ))
    }
}

/// Logical bytes per sector of a volume root (`GetDiskFreeSpaceW`).
pub fn volume_sector_size(mount_point: &str) -> Option<u32> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::volume_sector_size(mount_point)
    }
    #[cfg(not(windows))]
    {
        let _ = mount_point;
        None
    }
}

/// Model (`MSFT_PhysicalDisk.FriendlyName`) of the single physical disk backing a volume.
/// `None` when the volume spans several disks, the disk cannot be resolved, or WMI fails.
/// Slow (WMI): call it off latency-sensitive threads.
pub fn volume_disk_model(mount_point: &str) -> Option<String> {
    #[cfg(windows)]
    {
        crate::windows::bench_env::volume_disk_model(mount_point)
    }
    #[cfg(not(windows))]
    {
        let _ = mount_point;
        None
    }
}

/// Normalizes a user-supplied drive mount point to `X:\` (upper case). Accepts `c`, `c:`,
/// `C:\`, `C:/`. Anything else (UNC, paths, devices) is rejected.
pub fn normalize_drive_mount(input: &str) -> Option<String> {
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
    }
}
