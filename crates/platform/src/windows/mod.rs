//! Windows providers. All `unsafe` FFI of the crate lives below this module.

pub mod bench_env;
pub mod credentials;
pub mod devices;
pub mod dxgi;
pub mod gpu_bench;
pub mod kmt;
pub mod live;
pub mod nvml;
pub mod pdh;
pub mod pnp_wmi;
pub mod power;
pub mod process;
pub mod remediation;
pub mod time;
pub mod toolhelp;
pub mod wmi;
pub mod wua;

use models::hardware::{GpuInfo, GpuVendor};
use models::metric::{Metric, Source, Unavailability, Unit};

use crate::convert::gpu_id;

/// Nominal maximum CPU frequency: the highest `MaxMhz` across logical processors
/// (on hybrid CPUs P-cores and E-cores report different values).
pub fn cpu_max_mhz() -> Metric<f64> {
    match power::processor_power_info() {
        Ok(info) => match info.iter().map(|p| p.MaxMhz).filter(|v| *v > 0).max() {
            Some(v) => Metric::available(f64::from(v), Unit::Megahertz, Source::PowerApi),
            None => Metric::unavailable(Unavailability::NotSupported),
        },
        Err(e) => Metric::unavailable(Unavailability::Failed(e)),
    }
}

/// GPU inventory from DXGI, enriched with which telemetry providers serve each adapter.
pub fn gpu_inventory() -> Result<Vec<GpuInfo>, models::AppError> {
    let adapters = dxgi::enumerate_adapters()?;
    let nvml = nvml::init(&adapters);
    Ok(adapters
        .into_iter()
        .map(|a| {
            let mut telemetry_sources = Vec::new();
            // PDH reports every adapter except indirect-display (virtual display) ones.
            if !a.kmt.is_indirect_display() {
                telemetry_sources.push(Source::Pdh);
            }
            if let nvml::NvmlState::Ready(p) = &nvml
                && p.is_matched(a.luid)
            {
                telemetry_sources.push(Source::Nvml);
            }
            // Vendor-neutral WDDM sensors (temperature, clock, power share, fan).
            if !a.is_software
                && !a.kmt.is_indirect_display()
                && kmt::KmtAdapter::open(a.luid).is_ok_and(|k| {
                    let caps = k.caps();
                    k.perf(&caps)
                        .is_ok_and(|p| p.temperature_c.is_some() || p.core_clock_mhz.is_some())
                })
            {
                telemetry_sources.push(Source::D3dkmt);
            }
            GpuInfo {
                id: gpu_id(a.luid),
                name: a.name,
                vendor: GpuVendor::from_pci_id(a.vendor_id),
                vendor_id: a.vendor_id,
                device_id: Metric::available(a.device_id, Unit::None, Source::Dxgi),
                dedicated_vram_bytes: dxgi_bytes(a.dedicated_vram_bytes),
                shared_memory_bytes: dxgi_bytes(a.shared_memory_bytes),
                is_software: a.is_software,
                is_virtual: a.kmt.is_indirect_display(),
                driver_version: a.driver_version,
                telemetry_sources,
            }
        })
        .collect())
}

/// A DXGI memory size. DXGI always reports both sizes; 0 is a real value (e.g. the
/// software rasterizer has no memory of its own).
#[allow(clippy::cast_precision_loss)] // exact below 2^53 bytes
fn dxgi_bytes(v: u64) -> Metric<f64> {
    Metric::available(v as f64, Unit::Bytes, Source::Dxgi)
}
