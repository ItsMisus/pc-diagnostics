//! GPUs from the I/O Kit accelerator services (`IOAccelerator`), plus Metal for the
//! shared-memory budget. No privileges needed.
//!
//! Ids are `ioreg:<registry entry id, 16 hex digits>`: unique until the entry goes away
//! or the Mac reboots (like a Windows LUID), and identical in the inventory and in the
//! live samples.
//!
//! Inventory fields:
//! - `name`: accelerator `model` (e.g. "Apple M5").
//! - `vendor_id` / `device_id`: the PCI ids. Apple Silicon GPUs are not PCI devices:
//!   the accelerator carries `vendor-id` (0x106B, Apple) but there is no device id, so
//!   `device_id` is 0 there (the model field is not optional; see the integrator notes).
//!   Discrete/Intel GPUs on Intel Macs read both from the parent `IOPCIDevice`.
//! - `dedicated_vram_bytes`: Apple Silicon GPUs have no memory of their own (unified
//!   memory: CPU and GPU share the same RAM), so there is no dedicated VRAM and the
//!   value is 0. PCI GPUs report `VRAM,totalMB` when the driver publishes it.
//! - `shared_memory_bytes`: Metal's `recommendedMaxWorkingSetSize` for the same GPU
//!   (matched by `registryID`), the system-memory budget the OS lets the GPU use; the
//!   same meaning as DXGI `SharedSystemMemory`. 0 when Metal does not report the GPU.
//! - `driver_version`: `IOSourceVersion` of the accelerator's kernel driver bundle
//!   (e.g. `com.apple.AGXG17G` 362.5.1).
//!
//! Live fields: `usage_percent` is the accelerator's `PerformanceStatistics`
//! `Device Utilization %` (what Activity Monitor's GPU history shows: share of time the
//! GPU was busy). Not reported:
//! - dedicated memory in use: no dedicated memory exists on unified memory; the driver's
//!   "In use system memory" is shared RAM and would mislead under a "dedicated" label;
//! - core clock and power: only in the private IOReport framework or `powermetrics`
//!   (root);
//! - power share and fan: no such counters (Apple Silicon GPUs have no own fan).

use models::hardware::{GpuInfo, GpuVendor};
use models::metric::{Metric, Source, Unavailability, Unit};
use models::monitor::GpuSample;
use models::{AppError, ErrorCode};
use objc2_metal::{MTLCopyAllDevices, MTLDevice};

use super::iokit::{self, IoObject};

/// `GpuInfo::id` / `GpuSample::id` for an accelerator registry entry.
pub fn gpu_id(entry_id: u64) -> String {
    format!("ioreg:{entry_id:016X}")
}

/// Little-endian `u32` at the start of a registry `OSData` (PCI ids are stored so).
pub fn data_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?))
}

/// `Device Utilization %` from a `PerformanceStatistics` dictionary, clamped to 0–100.
fn utilization(stats: &iokit::Dict) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)] // a percentage
    let v = stats
        .number("Device Utilization %")
        .or_else(|| stats.number("Device Utilization"))? as f64;
    crate::convert::clamp_percent(v)
}

struct Accelerator {
    id: String,
    entry_id: u64,
    service: IoObject,
}

fn accelerators() -> Result<Vec<Accelerator>, AppError> {
    Ok(iokit::services_of_class(c"IOAccelerator")?
        .into_iter()
        .filter_map(|service| {
            let entry_id = service.entry_id()?;
            Some(Accelerator {
                id: gpu_id(entry_id),
                entry_id,
                service,
            })
        })
        .collect())
}

/// Metal's shared-memory budget per GPU registry id, for unified-memory GPUs.
fn metal_working_sets() -> Vec<(u64, u64, bool)> {
    let devices = MTLCopyAllDevices();
    (0..devices.count())
        .map(|i| {
            let d = devices.objectAtIndex(i);
            (
                d.registryID(),
                d.recommendedMaxWorkingSetSize(),
                d.hasUnifiedMemory(),
            )
        })
        .collect()
}

pub fn inventory() -> Result<Vec<GpuInfo>, AppError> {
    let accels = accelerators()?;
    let metal = metal_working_sets();
    Ok(accels
        .iter()
        .map(|a| {
            let s = &a.service;
            let parent = s.parent();
            let pci = parent.as_ref().filter(|p| p.conforms_to(c"IOPCIDevice"));
            let vendor_id = s
                .data("vendor-id")
                .or_else(|| pci.and_then(|p| p.data("vendor-id")))
                .and_then(|d| data_u32(&d))
                .unwrap_or(0);
            let device_id = pci
                .and_then(|p| p.data("device-id"))
                .and_then(|d| data_u32(&d))
                .unwrap_or(0);
            let name = s
                .string("model")
                .or_else(|| pci.and_then(|p| p.string("model")))
                .or_else(|| pci.and_then(|p| p.data("model")).map(|d| lossy_c(&d)))
                .unwrap_or_else(|| s.class_name().unwrap_or_default());
            let m = metal.iter().find(|(id, ..)| *id == a.entry_id);
            let unified = m.is_some_and(|(_, _, u)| *u) || pci.is_none();
            let vram_mb = s
                .number("VRAM,totalMB")
                .or_else(|| pci.and_then(|p| p.number("VRAM,totalMB")));
            let dedicated_vram_bytes = match (unified, vram_mb) {
                (false, Some(mb)) => u64::try_from(mb).unwrap_or(0).saturating_mul(1 << 20),
                _ => 0,
            };
            let shared_memory_bytes = match m {
                Some((_, ws, true)) => *ws,
                _ => 0,
            };
            let driver_version = match s.string("IOSourceVersion") {
                Some(v) if !v.trim().is_empty() => {
                    Metric::available(v.trim().to_owned(), Unit::None, Source::IoKit)
                }
                _ => Metric::unavailable(Unavailability::NotSupported),
            };
            let telemetry_sources = if s.dictionary("PerformanceStatistics").is_some() {
                vec![Source::IoKit]
            } else {
                Vec::new()
            };
            GpuInfo {
                id: a.id.clone(),
                name,
                vendor: GpuVendor::from_pci_id(vendor_id),
                vendor_id,
                device_id,
                dedicated_vram_bytes,
                shared_memory_bytes,
                is_software: false,
                is_virtual: false,
                driver_version,
                telemetry_sources,
            }
        })
        .collect())
}

fn lossy_c(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(bytes.get(..end).unwrap_or_default())
        .trim()
        .to_owned()
}

/// Per-tick GPU sampling over a cached accelerator list.
pub struct GpuLive {
    accels: Result<Vec<Accelerator>, AppError>,
    stale: bool,
}

impl GpuLive {
    pub fn new() -> Self {
        Self {
            accels: accelerators(),
            stale: false,
        }
    }

    pub fn samples(&mut self, gpu_temperature_c: Option<f64>) -> Vec<GpuSample> {
        if self.stale {
            self.accels = accelerators();
            self.stale = false;
        }
        let list = match &self.accels {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(error = %e, "GPU accelerator list failed");
                self.stale = true;
                return Vec::new();
            }
        };
        // A sensor reading can only be attributed when there is exactly one GPU.
        let temp = gpu_temperature_c.filter(|_| list.len() == 1);
        let mut stale = false;
        let out = list
            .iter()
            .map(|a| {
                let usage_percent = match a.service.dictionary("PerformanceStatistics") {
                    Some(stats) => match utilization(&stats) {
                        Some(v) => Metric::available(v, Unit::Percent, Source::IoKit),
                        None => Metric::unavailable(Unavailability::NotSupported),
                    },
                    None => {
                        stale = true;
                        Metric::unavailable(Unavailability::Failed(AppError::new(
                            ErrorCode::Platform,
                            "GPU PerformanceStatistics not readable",
                        )))
                    }
                };
                let na = || Metric::unavailable(Unavailability::NotSupported);
                GpuSample {
                    id: a.id.clone(),
                    usage_percent,
                    dedicated_used_bytes: na(),
                    temperature_c: match temp {
                        Some(t) => Metric::available(t, Unit::Celsius, Source::IoKit),
                        None => na(),
                    },
                    core_clock_mhz: na(),
                    power_w: Metric::unavailable(Unavailability::PermissionDenied),
                    power_percent: na(),
                    fan_rpm: na(),
                }
            })
            .collect();
        self.stale = stale;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_pci_data_decode() {
        assert_eq!(gpu_id(0x1_0000_050C), "ioreg:000000010000050C");
        assert_eq!(data_u32(&[0x6b, 0x10, 0, 0]), Some(0x106B));
        assert_eq!(data_u32(&[0x6b, 0x10]), None);
        assert_eq!(lossy_c(b"AMD Radeon Pro 5500M\0\0"), "AMD Radeon Pro 5500M");
    }

    #[test]
    fn host_gpu_inventory_matches_live_ids() {
        let inv = inventory().expect("GPU inventory");
        assert!(!inv.is_empty(), "every Mac has a GPU");
        let mut live = GpuLive::new();
        let samples = live.samples(None);
        for g in &inv {
            assert!(g.id.starts_with("ioreg:"));
            assert!(!g.name.is_empty());
            assert!(samples.iter().any(|s| s.id == g.id), "live id matches");
        }
        if cfg!(target_arch = "aarch64") {
            let g = &inv[0];
            assert_eq!(g.vendor_id, 0x106B, "Apple");
            assert_eq!(g.dedicated_vram_bytes, 0, "unified memory");
            assert!(g.shared_memory_bytes > 0, "Metal working-set budget");
            assert!(g.telemetry_sources.contains(&Source::IoKit));
            let u = samples[0].usage_percent.value().expect("utilization");
            assert!((0.0..=100.0).contains(u));
        }
    }
}
