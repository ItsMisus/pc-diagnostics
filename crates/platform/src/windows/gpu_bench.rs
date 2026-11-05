//! GPU benchmark support: direct NVML sensor for one adapter and a kill-on-close job
//! object for the isolated benchmark worker process.

use std::os::windows::io::AsRawHandle;

use ::windows::Win32::Foundation::{CloseHandle, HANDLE};
use ::windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use ::windows::core::PCWSTR;
use models::{AppError, ErrorCode};

use super::kmt::{KmtAdapter, KmtCaps};
use super::{dxgi, nvml};
use crate::convert::{Luid, gpu_id};
use crate::gpu_bench::{GpuReading, GpuThermalThresholds};

pub struct Sensor {
    /// NVML, when the adapter is an NVIDIA GPU NVML matched unambiguously.
    nvml: Option<Box<nvml::NvmlProvider>>,
    kmt: Option<(KmtAdapter, KmtCaps)>,
    luid: Luid,
}

impl Sensor {
    pub fn open(id: &str) -> Option<Self> {
        let adapters = dxgi::enumerate_adapters().ok()?;
        let a = adapters.iter().find(|a| gpu_id(a.luid) == id)?;
        if a.is_software || a.kmt.is_indirect_display() {
            return None;
        }
        let luid = a.luid;
        let kmt = KmtAdapter::open(luid).ok().map(|k| {
            let caps = k.caps();
            (k, caps)
        });
        let nvml = match nvml::init(&adapters) {
            nvml::NvmlState::Ready(p) if p.is_matched(luid) => Some(p),
            _ => None,
        };
        (nvml.is_some() || kmt.is_some()).then_some(Self { nvml, kmt, luid })
    }

    pub fn thresholds(&self) -> GpuThermalThresholds {
        let mut out = GpuThermalThresholds::default();
        if let Some(p) = &self.nvml {
            let t = p.thresholds(self.luid);
            out.slowdown_c = t.slowdown_c;
            out.shutdown_c = t.shutdown_c;
            out.gpu_max_c = t.gpu_max_c;
        }
        if let Some((_, caps)) = &self.kmt {
            out.driver_warning_c = caps.temperature_warning_c;
            out.driver_max_c = caps.temperature_max_c;
        }
        out
    }

    pub fn read(&self) -> GpuReading {
        let wddm = self.kmt.as_ref().and_then(|(k, caps)| k.perf(caps).ok());
        match &self.nvml {
            Some(p) => {
                let r = p.read(self.luid);
                GpuReading {
                    temperature_c: r
                        .temperature_c
                        .value()
                        .copied()
                        .or(wddm.and_then(|w| w.temperature_c)),
                    core_clock_mhz: r
                        .core_clock_mhz
                        .value()
                        .copied()
                        .or(wddm.and_then(|w| w.core_clock_mhz)),
                    power_w: r.power_w.value().copied(),
                }
            }
            None => GpuReading {
                temperature_c: wddm.and_then(|w| w.temperature_c),
                core_clock_mhz: wddm.and_then(|w| w.core_clock_mhz),
                power_w: None,
            },
        }
    }
}

fn hr_error(op: &str, e: &::windows::core::Error) -> AppError {
    AppError::new(ErrorCode::Platform, format!("{op} failed")).with_detail(format!(
        "HRESULT 0x{:08X}: {}",
        e.code().0,
        e.message()
    ))
}

/// Job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Owned handle.
pub struct Job(HANDLE);

// SAFETY: a job object handle is a process-wide kernel handle; it may be used and closed
// from any thread.
unsafe impl Send for Job {}
// SAFETY: as above; `Job` exposes no interior mutability.
unsafe impl Sync for Job {}

impl Job {
    pub fn assign(child: &std::process::Child) -> Result<Self, AppError> {
        // SAFETY: anonymous job, default security; the handle is owned by `Job` below.
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|e| hr_error("CreateJobObjectW", &e))?;
        let job = Self(handle);
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `info` is a valid JOBOBJECT_EXTENDED_LIMIT_INFORMATION and the size
        // passed matches its type, as the information class requires.
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).unwrap_or(0),
            )
        }
        .map_err(|e| hr_error("SetInformationJobObject", &e))?;
        let process = HANDLE(child.as_raw_handle());
        // SAFETY: `process` is the live handle owned by `child` (borrowed for the call);
        // `job.0` is the job handle created above.
        unsafe { AssignProcessToJobObject(job.0, process) }
            .map_err(|e| hr_error("AssignProcessToJobObject", &e))?;
        Ok(job)
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by CreateJobObjectW and is closed exactly once.
        // Closing the last handle kills any process still in the job.
        let _ = unsafe { CloseHandle(self.0) };
    }
}
