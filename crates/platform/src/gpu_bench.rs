//! Read-only facts and process plumbing for the GPU benchmark (crate `bench`):
//!
//! - [`gpu_adapters`]: the DXGI adapter list (same data and order as the inventory); on
//!   macOS the I/O Kit `IOAccelerator` list (ids from [`registry_gpu_id`]).
//! - [`GpuSensor`]: direct readings and the driver's own temperature thresholds for one
//!   physical adapter: NVML on NVIDIA when available, otherwise the vendor-neutral WDDM
//!   performance data (D3DKMT). Used when the live monitor is not running. macOS: none
//!   (Apple Silicon publishes no GPU temperature, clock or power to unprivileged
//!   processes through a public API; the benchmark's conservative duration cap applies).
//! - [`KillOnCloseJob`]: puts the benchmark worker process in a job object that the OS
//!   terminates when the app's handle closes, so a crashed app never leaves it behind.
//!   Windows only: on macOS the worker watches its parent itself (see `bench::gpu_worker`).

use models::AppError;
use models::hardware::GpuInfo;

/// The driver's temperature thresholds (°C). `None` = not exposed for this device.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GpuThermalThresholds {
    /// Hardware slowdown threshold (`NVML_TEMPERATURE_THRESHOLD_SLOWDOWN`).
    pub slowdown_c: Option<f64>,
    /// Hardware shutdown threshold (`NVML_TEMPERATURE_THRESHOLD_SHUTDOWN`).
    pub shutdown_c: Option<f64>,
    /// `NVML_TEMPERATURE_THRESHOLD_GPU_MAX`.
    pub gpu_max_c: Option<f64>,
    /// WDDM `TemperatureWarning` reported by the driver (any vendor; often absent on
    /// integrated GPUs).
    pub driver_warning_c: Option<f64>,
    /// WDDM `TemperatureMax` reported by the driver.
    pub driver_max_c: Option<f64>,
}

/// One direct sensor reading. `None` = not readable now.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GpuReading {
    pub temperature_c: Option<f64>,
    pub core_clock_mhz: Option<f64>,
    pub power_w: Option<f64>,
}

/// `GpuInfo::id` on macOS for an I/O Kit registry entry id: `ioreg:<16 upper-case hex>`.
/// The id is the `IOAccelerator` entry's, which Metal also reports as
/// `MTLDevice.registryID`. Defined on every OS so the format can be parsed and tested
/// anywhere.
pub fn registry_gpu_id(entry_id: u64) -> String {
    format!("ioreg:{entry_id:016X}")
}

/// Every DXGI adapter with its software/virtual flags (see `GpuInfo`). macOS: every
/// `IOAccelerator`, exactly as the hardware inventory lists it (`macos::gpu::inventory`; no
/// software or virtual adapters are listed there).
pub fn gpu_adapters() -> Result<Vec<GpuInfo>, AppError> {
    #[cfg(windows)]
    {
        crate::windows::gpu_inventory()
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::gpu::inventory()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Ok(Vec::new())
    }
}

/// Sensor for the adapter whose `GpuInfo::id` is `id`.
pub struct GpuSensor {
    #[cfg(windows)]
    inner: crate::windows::gpu_bench::Sensor,
}

impl GpuSensor {
    /// `None` when no adapter has this id, or it is software/virtual, or neither NVML nor
    /// WDDM performance data can be opened for it. Always `None` on macOS (see the module
    /// docs).
    pub fn open(id: &str) -> Option<Self> {
        #[cfg(windows)]
        {
            crate::windows::gpu_bench::Sensor::open(id).map(|inner| Self { inner })
        }
        #[cfg(not(windows))]
        {
            let _ = id;
            None
        }
    }

    pub fn thresholds(&self) -> GpuThermalThresholds {
        #[cfg(windows)]
        {
            self.inner.thresholds()
        }
        #[cfg(not(windows))]
        {
            GpuThermalThresholds::default()
        }
    }

    pub fn read(&self) -> GpuReading {
        #[cfg(windows)]
        {
            self.inner.read()
        }
        #[cfg(not(windows))]
        {
            GpuReading::default()
        }
    }
}

/// Job object that kills its processes when dropped (or when the app exits).
pub struct KillOnCloseJob {
    #[cfg(windows)]
    _job: crate::windows::gpu_bench::Job,
}

impl KillOnCloseJob {
    /// Creates the job and assigns `child` to it. `NotSupported` outside Windows.
    pub fn assign(child: &std::process::Child) -> Result<Self, AppError> {
        #[cfg(windows)]
        {
            crate::windows::gpu_bench::Job::assign(child).map(|j| Self { _job: j })
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Err(AppError::new(
                models::ErrorCode::NotSupported,
                "job objects are Windows-only",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_ids_are_fixed_width_hex() {
        assert_eq!(registry_gpu_id(0x1_0000_050C), "ioreg:000000010000050C");
        assert_eq!(registry_gpu_id(u64::MAX), "ioreg:FFFFFFFFFFFFFFFF");
    }

    /// Host test: on this Mac the Apple GPU is listed with a registry id.
    #[cfg(target_os = "macos")]
    #[test]
    fn host_lists_the_apple_gpu() {
        let gpus = gpu_adapters().expect("adapters");
        eprintln!("{gpus:#?}");
        assert!(!gpus.is_empty(), "at least one IOAccelerator");
        for g in &gpus {
            assert!(g.id.starts_with("ioreg:") && g.id.len() == 22, "{}", g.id);
            assert!(!g.name.is_empty());
            assert!(!g.is_software && !g.is_virtual);
        }
        if cfg!(target_arch = "aarch64") {
            assert!(
                gpus.iter()
                    .any(|g| g.vendor_id == 0x106B && g.name.starts_with("Apple"))
            );
        }
        assert!(GpuSensor::open(&gpus[0].id).is_none());
    }
}
