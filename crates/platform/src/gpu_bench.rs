//! Read-only facts and process plumbing for the GPU benchmark (crate `bench`):
//!
//! - [`gpu_adapters`]: the DXGI adapter list (same data and order as the inventory).
//! - [`GpuSensor`]: direct readings and the driver's own temperature thresholds for one
//!   physical adapter: NVML on NVIDIA when available, otherwise the vendor-neutral WDDM
//!   performance data (D3DKMT). Used when the live monitor is not running.
//! - [`KillOnCloseJob`]: puts the benchmark worker process in a job object that the OS
//!   terminates when the app's handle closes, so a crashed app never leaves it behind.

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

/// Every DXGI adapter with its software/virtual flags (see `GpuInfo`).
pub fn gpu_adapters() -> Result<Vec<GpuInfo>, AppError> {
    #[cfg(windows)]
    {
        crate::windows::gpu_inventory()
    }
    #[cfg(not(windows))]
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
    /// WDDM performance data can be opened for it.
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
    /// Creates the job and assigns `child` to it.
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
