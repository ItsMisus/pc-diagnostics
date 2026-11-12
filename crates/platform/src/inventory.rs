//! Hardware inventory. Slow (WMI, DXGI, NVML init): call on demand and cache the result;
//! never call it from a per-second loop.
//!
//! macOS: CPU from `sysinfo` plus the rated maximum from `macos::cpu`; GPUs from the
//! I/O Kit accelerators (`macos::gpu`); physical disks from the I/O Kit block storage
//! drivers (`macos::storage`); volumes from `sysinfo` without the boot container's
//! system-managed mounts (see [`volumes`]). Memory modules are not exposed (see
//! [`collect_inventory`]).

use models::hardware::{CpuInfo, GpuInfo, HardwareInventory, MediaKind, MemoryInfo, VolumeInfo};
use models::metric::{Metric, Unavailability};
use models::time::now_unix_ms;
use sysinfo::{CpuRefreshKind, DiskKind, Disks, MemoryRefreshKind, RefreshKind, System};

use crate::convert::non_empty;

pub fn collect_inventory() -> HardwareInventory {
    // WMI is the slowest part: run it concurrently with the rest.
    #[cfg(windows)]
    let wmi = std::thread::Builder::new()
        .name("inventory-wmi".into())
        .spawn(crate::windows::wmi::collect);

    let cpu = cpu_info();
    let gpus = gpu_info();
    let volumes = volumes();
    let total_bytes = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
    )
    .map_or(0, |s| s.total_memory());

    #[cfg(windows)]
    let (modules, physical_disks, form_factor) = {
        let (modules, physical_disks, chassis) = match wmi.map(|h| h.join()) {
            Ok(Ok(w)) => (w.modules, w.physical_disks, w.chassis_types),
            _ => {
                tracing::error!("WMI inventory thread failed");
                let r = Unavailability::Failed(models::AppError::internal(
                    "WMI inventory thread failed",
                ));
                (
                    Metric::unavailable(r.clone()),
                    Metric::unavailable(r.clone()),
                    Err(r),
                )
            }
        };
        let battery = crate::windows::bench_env::internal_battery();
        (
            modules,
            physical_disks,
            crate::form_factor::windows_info(chassis, battery),
        )
    };
    #[cfg(not(windows))]
    let form_factor = crate::form_factor::detect();
    // macOS has no public API for per-module memory facts (size, speed, part number per
    // DIMM): only `system_profiler` prints them, and on Apple Silicon the memory is in the
    // SoC package, not in modules. So the list stays unavailable rather than empty.
    #[cfg(target_os = "macos")]
    let (modules, physical_disks) = (
        Metric::unavailable(Unavailability::NotSupported),
        match crate::macos::storage::physical_disks() {
            Ok(d) => {
                Metric::available(d, models::metric::Unit::None, models::metric::Source::IoKit)
            }
            Err(e) => {
                tracing::error!(error = %e, "physical disk inventory failed");
                Metric::unavailable(Unavailability::Failed(e))
            }
        },
    );
    #[cfg(not(any(windows, target_os = "macos")))]
    let (modules, physical_disks) = (
        Metric::unavailable(Unavailability::NotSupported),
        Metric::unavailable(Unavailability::NotSupported),
    );

    HardwareInventory {
        collected_at: now_unix_ms(),
        cpu,
        gpus,
        memory: MemoryInfo {
            total_bytes,
            modules,
        },
        physical_disks,
        volumes,
        form_factor,
    }
}

fn cpu_info() -> CpuInfo {
    let sys =
        System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing()));
    let (vendor, brand, logical) = match &sys {
        Ok(s) => {
            let first = s.cpus().first();
            (
                non_empty(first.map(|c| c.vendor_id().to_owned())),
                non_empty(first.map(|c| c.brand().to_owned())),
                s.cpus().len(),
            )
        }
        Err(e) => {
            tracing::warn!(error = %e, "sysinfo CPU list failed");
            (None, None, 0)
        }
    };
    let os_logical = crate::overhead::logical_cpu_count();
    let logical = if os_logical > 0 { os_logical } else { logical };
    #[cfg(windows)]
    let max_mhz = crate::windows::cpu_max_mhz();
    // Rated maximum (Intel: `hw.cpufrequency_max`; Apple Silicon: the performance
    // cluster's top DVFS state), the same value the live sample reports.
    #[cfg(target_os = "macos")]
    let max_mhz = crate::macos::cpu::max_mhz();
    #[cfg(not(any(windows, target_os = "macos")))]
    let max_mhz = Metric::unavailable(Unavailability::NotSupported);
    CpuInfo {
        vendor,
        brand,
        arch: System::cpu_arch(),
        physical_cores: System::physical_core_count()
            .ok()
            .and_then(|n| u32::try_from(n).ok()),
        logical_cores: u32::try_from(logical).unwrap_or(u32::MAX),
        max_mhz,
    }
}

fn gpu_info() -> Vec<GpuInfo> {
    #[cfg(windows)]
    {
        crate::windows::gpu_inventory().unwrap_or_else(|e| {
            tracing::error!(error = %e, "GPU inventory failed");
            Vec::new()
        })
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::gpu::inventory().unwrap_or_else(|e| {
            tracing::error!(error = %e, "GPU inventory failed");
            Vec::new()
        })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

/// Whether a mount point is listed as a volume.
///
/// macOS: the boot disk is one APFS container mounted as the read-only system snapshot
/// at `/` plus system-managed volumes under `/System/Volumes/` (`Data` holds the user's
/// files; `VM`, `Preboot`, `Update`, … are internal). They all share the container's
/// space, so `/` alone (shown by Finder as "Macintosh HD") already reports it; listing
/// `/System/Volumes/Data` too would show the same space twice.
fn is_listed_volume(mount_point: &str) -> bool {
    if cfg!(target_os = "macos") {
        !mount_point.starts_with("/System/Volumes/")
    } else {
        true
    }
}

fn volumes() -> Vec<VolumeInfo> {
    let disks = match Disks::new_with_refreshed_list() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, "volume enumeration failed");
            return Vec::new();
        }
    };
    disks
        .iter()
        .filter(|d| is_listed_volume(&d.mount_point().to_string_lossy()))
        .map(|d| VolumeInfo {
            mount_point: d.mount_point().to_string_lossy().into_owned(),
            label: non_empty(Some(d.name().to_string_lossy().into_owned())),
            file_system: non_empty(Some(d.file_system().to_string_lossy().into_owned())),
            total_bytes: d.total_space(),
            available_bytes: d.available_space(),
            removable: d.is_removable(),
            media: match d.kind() {
                DiskKind::SSD => MediaKind::Ssd,
                DiskKind::HDD => MediaKind::Hdd,
                DiskKind::Unknown(_) => MediaKind::Unknown,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Host smoke test: real collection, invariants only.
    #[test]
    fn host_inventory_is_plausible() {
        let inv = collect_inventory();
        assert!(inv.cpu.logical_cores >= 1);
        assert!(!inv.cpu.arch.is_empty());
        assert!(inv.memory.total_bytes > 0);
        if let Some(mods) = inv.memory.modules.value() {
            // Virtual machines may expose no modules; real ones must have a capacity.
            assert!(mods.iter().all(|m| m.capacity_bytes > 0));
        }
        if cfg!(windows) {
            assert!(!inv.gpus.is_empty(), "DXGI always reports at least WARP");
            assert!(inv.gpus.iter().all(|g| g.id.starts_with("luid:")));
            assert!(!inv.volumes.is_empty());
        }
        for v in &inv.volumes {
            assert!(v.available_bytes <= v.total_bytes);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn volume_filter_drops_boot_container_system_mounts() {
        assert!(is_listed_volume("/"));
        assert!(is_listed_volume("/Volumes/USB"));
        assert!(!is_listed_volume("/System/Volumes/Data"));
        assert!(!is_listed_volume("/System/Volumes/VM"));
    }

    // This Mac: the GPU, the boot disk and "/" must be real; what macOS does not expose
    // must stay unavailable.
    #[cfg(target_os = "macos")]
    #[test]
    fn host_macos_inventory_is_plausible() {
        let inv = collect_inventory();
        assert!(!inv.gpus.is_empty(), "every Mac has a GPU");
        assert!(inv.gpus.iter().all(|g| g.id.starts_with("ioreg:")));
        assert!(
            !inv.memory.modules.is_available(),
            "no public per-module API"
        );
        let disks = inv.physical_disks.value().expect("I/O Kit disks");
        assert!(disks.iter().any(|d| d.id == "disk0" && d.size_bytes > 0));
        let root = inv
            .volumes
            .iter()
            .find(|v| v.mount_point == "/")
            .expect("/ listed");
        assert!(root.total_bytes > 10_000_000_000 && !root.removable);
        assert!(
            inv.volumes
                .iter()
                .all(|v| !v.mount_point.starts_with("/System/Volumes/"))
        );
        if cfg!(target_arch = "aarch64") {
            assert_eq!(inv.cpu.vendor.as_deref(), Some("Apple"));
            let m = *inv.cpu.max_mhz.value().expect("rated max MHz");
            assert!((2000.0..=6000.0).contains(&m), "{m} MHz");
            let g = &inv.gpus[0];
            assert_eq!(g.vendor_id, 0x106B);
            assert!(!g.dedicated_vram_bytes.is_available(), "unified memory");
            assert!(g.name.starts_with("Apple"), "{}", g.name);
        }
    }
}
