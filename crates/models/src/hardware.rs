//! Hardware inventory: slow-changing facts, collected on demand and cached.
//!
//! Byte sizes are `u64` exported to TypeScript as `number` (exact below 2^53 = 8 PiB).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::device::FormFactorInfo;
use crate::metric::{Metric, Source};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CpuInfo {
    pub vendor: Option<String>,
    pub brand: Option<String>,
    pub arch: String,
    pub physical_cores: Option<u32>,
    pub logical_cores: u32,
    /// Nominal maximum frequency (`CallNtPowerInformation` `MaxMhz`).
    pub max_mhz: Metric<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Microsoft,
    /// Apple (PCI vendor id 0x106B): Apple silicon GPUs and the GPUs Apple names on Intel Macs.
    Apple,
    Other,
}

impl GpuVendor {
    /// Maps a PCI vendor id to a vendor.
    pub fn from_pci_id(id: u32) -> Self {
        match id {
            0x10DE => Self::Nvidia,
            0x1002 | 0x1022 => Self::Amd,
            0x8086 => Self::Intel,
            0x1414 => Self::Microsoft,
            0x106B => Self::Apple,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GpuInfo {
    /// Adapter LUID as hex (`luid:<high>-<low>`). Stable until reboot; joins inventory to samples.
    pub id: String,
    pub name: String,
    pub vendor: GpuVendor,
    pub vendor_id: u32,
    /// PCI device id. `NotSupported` for GPUs that are not PCI devices (Apple silicon).
    pub device_id: Metric<u32>,
    /// Memory that belongs to the GPU alone. `NotSupported` on unified-memory GPUs (Apple
    /// silicon), where the processor and the GPU share the same RAM: there is none.
    pub dedicated_vram_bytes: Metric<f64>,
    /// System RAM the OS lets the GPU use: DXGI `SharedSystemMemory` on Windows, Metal
    /// `recommendedMaxWorkingSetSize` on macOS (the GPU's working-set budget).
    pub shared_memory_bytes: Metric<f64>,
    /// Software rasterizer (e.g. Microsoft Basic Render Driver).
    pub is_software: bool,
    /// Indirect-display (virtual display) adapter, e.g. remote-desktop/streaming drivers.
    /// It may duplicate a real GPU's name and PCI ids but exposes no telemetry.
    pub is_virtual: bool,
    pub driver_version: Metric<String>,
    /// Providers that deliver live telemetry for this adapter (e.g. `pdh`, `nvml`).
    pub telemetry_sources: Vec<Source>,
}

/// One DIMM/SODIMM. Serial numbers are deliberately not collected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryModule {
    #[ts(type = "number")]
    pub capacity_bytes: u64,
    /// Rated speed (`Win32_PhysicalMemory.Speed`).
    pub speed_mts: Option<u32>,
    /// Configured speed (`ConfiguredClockSpeed`).
    pub configured_speed_mts: Option<u32>,
    pub manufacturer: Option<String>,
    pub part_number: Option<String>,
    pub form_factor: Option<String>,
    pub slot: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryInfo {
    #[ts(type = "number")]
    pub total_bytes: u64,
    pub modules: Metric<Vec<MemoryModule>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MediaKind {
    Ssd,
    Hdd,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BusKind {
    Nvme,
    Sata,
    Usb,
    Sas,
    Raid,
    Virtual,
    Other,
}

/// Windows Storage health status (`MSFT_PhysicalDisk.HealthStatus`). This is the OS
/// summary, not a full SMART/NVMe log evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DiskHealthStatus {
    Healthy,
    Warning,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PhysicalDiskInfo {
    /// `MSFT_PhysicalDisk.DeviceId` (disk number).
    pub id: String,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub media: MediaKind,
    pub bus: BusKind,
    #[ts(type = "number")]
    pub size_bytes: u64,
    pub health: Metric<DiskHealthStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeInfo {
    pub mount_point: String,
    pub label: Option<String>,
    pub file_system: Option<String>,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub available_bytes: u64,
    pub removable: bool,
    pub media: MediaKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HardwareInventory {
    #[ts(type = "number")]
    pub collected_at: u64,
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub memory: MemoryInfo,
    pub physical_disks: Metric<Vec<PhysicalDiskInfo>>,
    pub volumes: Vec<VolumeInfo>,
    /// Tower / laptop / mini PC ..., with the evidence used (see `FormFactorInfo`).
    /// Collected with the rest of the inventory, never per tick.
    pub form_factor: FormFactorInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_vendor_ids() {
        assert_eq!(GpuVendor::from_pci_id(0x10DE), GpuVendor::Nvidia);
        assert_eq!(GpuVendor::from_pci_id(0x1002), GpuVendor::Amd);
        assert_eq!(GpuVendor::from_pci_id(0x8086), GpuVendor::Intel);
        assert_eq!(GpuVendor::from_pci_id(0x1414), GpuVendor::Microsoft);
        assert_eq!(GpuVendor::from_pci_id(0x106B), GpuVendor::Apple);
        assert_eq!(GpuVendor::from_pci_id(0x1234), GpuVendor::Other);
    }
}
