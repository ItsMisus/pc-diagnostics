//! Hardware inventory. Slow (WMI, DXGI, NVML init): call on demand and cache the result;
//! never call it from a per-second loop.

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
    let (modules, physical_disks) = match wmi.map(|h| h.join()) {
        Ok(Ok(w)) => (w.modules, w.physical_disks),
        _ => {
            tracing::error!("WMI inventory thread failed");
            let r =
                Unavailability::Failed(models::AppError::internal("WMI inventory thread failed"));
            (Metric::unavailable(r.clone()), Metric::unavailable(r))
        }
    };
    #[cfg(not(windows))]
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
    #[cfg(not(windows))]
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
    #[cfg(not(windows))]
    {
        Vec::new()
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
}
