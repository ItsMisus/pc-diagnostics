//! Optional NVIDIA telemetry through NVML (`nvml.dll`, shipped with the NVIDIA driver).
//!
//! The library is loaded at runtime from an absolute path (System32, then the legacy NVSMI
//! folder) so a DLL planted next to the executable is never picked up.
//!
//! NVML devices are matched to DXGI adapters by PCI location (bus/device/function from
//! D3DKMT), falling back to the PCI vendor+device id only when it is unique on both
//! sides; see `convert::match_nvml_devices`. Ambiguous adapters are reported as a failed
//! match instead of guessing.

use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use ::windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use models::metric::{Metric, Source, Unavailability, Unit};
use models::{AppError, ErrorCode};
use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor, TemperatureThreshold};
use nvml_wrapper::error::NvmlError;

use super::dxgi::Adapter;
use crate::convert::{
    DxgiGpuKey, Luid, NvmlGpuKey, gpu_id, match_nvml_devices, parse_pci_function,
};

pub const NVML_DLL: &str = "nvml.dll";
const NVIDIA_PCI_VENDOR: u32 = 0x10DE;

pub enum NvmlState {
    Ready(Box<NvmlProvider>),
    Missing,
    Failed(AppError),
}

pub struct NvmlProvider {
    nvml: Nvml,
    /// DXGI LUID -> NVML device index.
    by_luid: HashMap<Luid, u32>,
}

fn candidate_paths() -> Vec<OsString> {
    let mut out = Vec::new();
    let mut buf = [0u16; 260];
    // SAFETY: `buf` is a valid writable slice; the call writes at most its length.
    let n = unsafe { GetSystemDirectoryW(Some(&mut buf)) } as usize;
    if n > 0 && n < buf.len() {
        let mut p = OsString::from_wide(&buf[..n]);
        p.push("\\nvml.dll");
        out.push(p);
    }
    if let Some(pf) = std::env::var_os("ProgramW6432").or_else(|| std::env::var_os("ProgramFiles"))
    {
        let mut p = pf;
        p.push("\\NVIDIA Corporation\\NVSMI\\nvml.dll");
        out.push(p);
    }
    out
}

pub fn nvml_error(op: &str, e: &NvmlError) -> AppError {
    AppError::new(ErrorCode::Platform, format!("NVML {op} failed")).with_detail(e.to_string())
}

fn reason(op: &str, e: &NvmlError) -> Unavailability {
    match e {
        NvmlError::NotSupported => Unavailability::NotSupported,
        NvmlError::NoPermission => Unavailability::PermissionDenied,
        _ => Unavailability::Failed(nvml_error(op, e)),
    }
}

/// Loads NVML only when DXGI reports at least one NVIDIA adapter.
pub fn init(adapters: &[Adapter]) -> NvmlState {
    if !adapters.iter().any(|a| a.vendor_id == NVIDIA_PCI_VENDOR) {
        return NvmlState::Missing;
    }
    let mut last_err = None;
    let mut nvml = None;
    for path in candidate_paths() {
        if !std::path::Path::new(&path).is_file() {
            continue;
        }
        match Nvml::builder().lib_path(&path).init() {
            Ok(n) => {
                nvml = Some(n);
                break;
            }
            Err(e) => last_err = Some(e),
        }
    }
    let Some(nvml) = nvml else {
        return match last_err {
            None | Some(NvmlError::LibloadingError(_)) => NvmlState::Missing,
            Some(e) => NvmlState::Failed(nvml_error("init", &e)),
        };
    };
    let by_luid = match match_devices(&nvml, adapters) {
        Ok(m) => m,
        Err(e) => return NvmlState::Failed(e),
    };
    NvmlState::Ready(Box::new(NvmlProvider { nvml, by_luid }))
}

fn match_devices(nvml: &Nvml, adapters: &[Adapter]) -> Result<HashMap<Luid, u32>, AppError> {
    let count = nvml
        .device_count()
        .map_err(|e| nvml_error("device_count", &e))?;
    let mut keys = Vec::new();
    for index in 0..count {
        match nvml.device_by_index(index).and_then(|d| d.pci_info()) {
            Ok(pci) => keys.push(NvmlGpuKey {
                index,
                pci_id: pci.pci_device_id,
                location: parse_pci_function(&pci.bus_id).map(|f| (pci.bus, pci.device, f)),
            }),
            Err(e) => tracing::warn!(index, error = %e, "NVML pci_info failed"),
        }
    }
    let dxgi: Vec<DxgiGpuKey> = adapters
        .iter()
        .filter(|a| a.vendor_id == NVIDIA_PCI_VENDOR && !a.is_software)
        .map(|a| DxgiGpuKey {
            luid: a.luid,
            pci_id: (a.device_id << 16) | a.vendor_id,
            location: a.kmt.pci.map(|p| (p.bus, p.device, p.function)),
            indirect_display: a.kmt.is_indirect_display(),
        })
        .collect();
    let out = match_nvml_devices(&dxgi, &keys);
    for d in dxgi
        .iter()
        .filter(|d| !d.indirect_display && !out.contains_key(&d.luid))
    {
        tracing::warn!(
            id = gpu_id(d.luid),
            "no unambiguous NVML device for this adapter"
        );
    }
    Ok(out)
}

pub struct NvmlReading {
    pub temperature_c: Metric<f64>,
    pub core_clock_mhz: Metric<f64>,
    pub power_w: Metric<f64>,
}

/// The driver's own temperature thresholds for one GPU (°C), as NVML reports them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NvmlThresholds {
    /// `NVML_TEMPERATURE_THRESHOLD_SLOWDOWN`: hardware slowdown starts here.
    pub slowdown_c: Option<f64>,
    /// `NVML_TEMPERATURE_THRESHOLD_SHUTDOWN`: hardware protection shutdown.
    pub shutdown_c: Option<f64>,
    /// `NVML_TEMPERATURE_THRESHOLD_GPU_MAX`: clocks may drop below base from here.
    pub gpu_max_c: Option<f64>,
}

impl NvmlProvider {
    pub fn is_matched(&self, luid: Luid) -> bool {
        self.by_luid.contains_key(&luid)
    }

    /// Thresholds of the NVML device matched to `luid`. A missing value means NVML does
    /// not expose it for this device (or the read failed); nothing is substituted.
    pub fn thresholds(&self, luid: Luid) -> NvmlThresholds {
        let Some(device) = self
            .by_luid
            .get(&luid)
            .and_then(|&i| self.nvml.device_by_index(i).ok())
        else {
            return NvmlThresholds::default();
        };
        let read = |t: TemperatureThreshold| match device.temperature_threshold(t) {
            Ok(v) if v > 0 => Some(f64::from(v)),
            Ok(_) => None,
            Err(e) => {
                tracing::debug!(?t, error = %e, "NVML temperature threshold unavailable");
                None
            }
        };
        NvmlThresholds {
            slowdown_c: read(TemperatureThreshold::Slowdown),
            shutdown_c: read(TemperatureThreshold::Shutdown),
            gpu_max_c: read(TemperatureThreshold::GpuMax),
        }
    }

    pub fn read(&self, luid: Luid) -> NvmlReading {
        let Some(&idx) = self.by_luid.get(&luid) else {
            let err = AppError::new(
                ErrorCode::NotFound,
                "No unambiguous NVML device matches this DXGI adapter",
            )
            .with_detail(gpu_id(luid));
            let r = Unavailability::Failed(err);
            return NvmlReading {
                temperature_c: Metric::unavailable(r.clone()),
                core_clock_mhz: Metric::unavailable(r.clone()),
                power_w: Metric::unavailable(r),
            };
        };
        let device = match self.nvml.device_by_index(idx) {
            Ok(d) => d,
            Err(e) => {
                let r = reason("device_by_index", &e);
                return NvmlReading {
                    temperature_c: Metric::unavailable(r.clone()),
                    core_clock_mhz: Metric::unavailable(r.clone()),
                    power_w: Metric::unavailable(r),
                };
            }
        };
        let temperature_c = match device.temperature(TemperatureSensor::Gpu) {
            Ok(t) => Metric::available(f64::from(t), Unit::Celsius, Source::Nvml),
            Err(e) => Metric::unavailable(reason("temperature", &e)),
        };
        let core_clock_mhz = match device.clock_info(Clock::Graphics) {
            Ok(c) => Metric::available(f64::from(c), Unit::Megahertz, Source::Nvml),
            Err(e) => Metric::unavailable(reason("clock_info", &e)),
        };
        let power_w = match device.power_usage() {
            Ok(mw) => Metric::available(f64::from(mw) / 1000.0, Unit::Watts, Source::Nvml),
            Err(e) => Metric::unavailable(reason("power_usage", &e)),
        };
        NvmlReading {
            temperature_c,
            core_clock_mhz,
            power_w,
        }
    }
}
