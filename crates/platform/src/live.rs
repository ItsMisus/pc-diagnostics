//! Live (per-tick) collection. Cheap by design: no WMI, no process enumeration, no
//! adapter enumeration inside `sample()`.
//!
//! `LiveCollector` holds raw OS handles and is meant to live on a single sampler thread.

use models::metric::{Metric, Source, Unavailability, Unit};
use models::monitor::{CpuSample, LiveSample, MemorySample, StorageSample};
use models::time::now_unix_ms;
use models::{AppError, ErrorCode};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

/// Reason for CPU temperature and package power on Windows.
pub const CPU_SENSOR_PROVIDER: &str =
    "CPU sensor provider (requires a kernel-mode driver; not bundled)";

pub struct LiveCollector {
    sys: Result<System, Unavailability>,
    #[cfg(windows)]
    win: crate::windows::live::WinLive,
}

fn cpu_sensor_reason() -> Unavailability {
    if cfg!(windows) {
        Unavailability::ProviderMissing(CPU_SENSOR_PROVIDER.into())
    } else {
        Unavailability::NotSupported
    }
}

impl LiveCollector {
    /// Opens providers and takes the baseline reading for rate counters (CPU usage, PDH
    /// rates). The first `sample()` should come at least ~200 ms later.
    ///
    /// On Windows CPU usage comes from PDH raw counters (`sysinfo`'s Windows CPU path
    /// reports `0.0` when PDH flags a value invalid, which would be a fabricated reading);
    /// `sysinfo` is used only for RAM there. Elsewhere `sysinfo` also provides CPU usage.
    ///
    /// Takes ~0.4 s on Windows (PDH loads the processor performance objects), so call it
    /// on the sampler thread, not on a latency-sensitive one.
    pub fn new() -> Self {
        let cpu_kind = if cfg!(windows) {
            None
        } else {
            Some(CpuRefreshKind::nothing().with_usage())
        };
        let mut kind = RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram());
        if let Some(c) = cpu_kind {
            kind = kind.with_cpu(c);
        }
        let sys = System::new_with_specifics(kind).map_err(|e| {
            tracing::warn!(error = %e, "sysinfo init failed");
            Unavailability::Failed(
                AppError::new(ErrorCode::Platform, "sysinfo init failed")
                    .with_detail(e.to_string()),
            )
        });
        Self {
            sys,
            #[cfg(windows)]
            win: crate::windows::live::WinLive::new(),
        }
    }

    /// Collects one sample. Every field is either measured or carries its reason.
    pub fn sample(&mut self, seq: u64) -> LiveSample {
        let at = now_unix_ms();
        #[cfg(windows)]
        self.win.collect();
        LiveSample {
            seq,
            at,
            cpu: self.cpu(),
            memory: self.memory(),
            gpus: self.gpus(),
            storage: self.storage(),
        }
    }

    #[cfg(windows)]
    fn cpu_usage(&mut self) -> (Metric<f64>, Metric<Vec<f64>>) {
        self.win.cpu_usage()
    }

    #[cfg(not(windows))]
    fn cpu_usage(&mut self) -> (Metric<f64>, Metric<Vec<f64>>) {
        let sys = match &mut self.sys {
            Ok(sys) => sys,
            Err(r) => {
                return (
                    Metric::unavailable(r.clone()),
                    Metric::unavailable(r.clone()),
                );
            }
        };
        sys.refresh_cpu_usage();
        let cores: Option<Vec<f64>> = sys
            .cpus()
            .iter()
            .map(|c| crate::convert::clamp_percent(f64::from(c.usage())))
            .collect();
        let usage = match crate::convert::clamp_percent(f64::from(sys.global_cpu_usage())) {
            Some(v) => Metric::available(v, Unit::Percent, Source::Sysinfo),
            None => Metric::unavailable(failed("CPU usage is not a finite number")),
        };
        let per_core = match cores {
            Some(c) if !c.is_empty() => Metric::available(c, Unit::Percent, Source::Sysinfo),
            Some(_) => Metric::unavailable(Unavailability::NotSupported),
            None => Metric::unavailable(failed("per-core usage is not a finite number")),
        };
        (usage, per_core)
    }

    fn cpu(&mut self) -> CpuSample {
        let (usage_percent, per_core_percent) = self.cpu_usage();
        #[cfg(windows)]
        let (current_mhz, max_mhz, limit_mhz) = self.win.cpu_frequency();
        #[cfg(not(windows))]
        let (current_mhz, max_mhz, limit_mhz) = (
            Metric::unavailable(Unavailability::NotSupported),
            Metric::unavailable(Unavailability::NotSupported),
            Metric::unavailable(Unavailability::NotSupported),
        );
        CpuSample {
            usage_percent,
            per_core_percent,
            current_mhz,
            max_mhz,
            limit_mhz,
            temperature_c: Metric::unavailable(cpu_sensor_reason()),
            power_w: Metric::unavailable(cpu_sensor_reason()),
        }
    }

    fn memory(&mut self) -> MemorySample {
        let (total_bytes, used_bytes, available_bytes) = match &mut self.sys {
            Ok(sys) => {
                sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
                let total = sys.total_memory();
                let avail = sys.available_memory();
                if total == 0 {
                    let r = failed("total memory reported as 0");
                    (
                        Metric::unavailable(r.clone()),
                        Metric::unavailable(r.clone()),
                        Metric::unavailable(r),
                    )
                } else {
                    #[allow(clippy::cast_precision_loss)] // exact below 2^53 bytes
                    let m = |v: u64| Metric::available(v as f64, Unit::Bytes, Source::Sysinfo);
                    (m(total), m(total.saturating_sub(avail)), m(avail))
                }
            }
            Err(r) => (
                Metric::unavailable(r.clone()),
                Metric::unavailable(r.clone()),
                Metric::unavailable(r.clone()),
            ),
        };
        #[cfg(windows)]
        let (commit_used_bytes, commit_limit_bytes, pages_per_sec) = self.win.memory_paging();
        #[cfg(not(windows))]
        let (commit_used_bytes, commit_limit_bytes, pages_per_sec) = (
            Metric::unavailable(Unavailability::NotSupported),
            Metric::unavailable(Unavailability::NotSupported),
            Metric::unavailable(Unavailability::NotSupported),
        );
        MemorySample {
            total_bytes,
            used_bytes,
            available_bytes,
            commit_used_bytes,
            commit_limit_bytes,
            pages_per_sec,
        }
    }

    fn gpus(&self) -> Vec<models::monitor::GpuSample> {
        #[cfg(windows)]
        return self.win.gpus();
        #[cfg(not(windows))]
        return Vec::new();
    }

    fn storage(&self) -> StorageSample {
        #[cfg(windows)]
        return self.win.storage();
        #[cfg(not(windows))]
        return StorageSample {
            read_bytes_per_sec: Metric::unavailable(Unavailability::NotSupported),
            write_bytes_per_sec: Metric::unavailable(Unavailability::NotSupported),
            active_time_percent: Metric::unavailable(Unavailability::NotSupported),
        };
    }
}

impl Default for LiveCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn failed(message: &str) -> Unavailability {
    Unavailability::Failed(AppError::new(ErrorCode::Platform, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Host smoke test: real collection, invariants only.
    #[test]
    fn host_sample_has_plausible_values() {
        let mut c = LiveCollector::new();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let s = c.sample(7);
        assert_eq!(s.seq, 7);
        assert!(s.at > 1_600_000_000_000);
        let total = *s.memory.total_bytes.value().expect("total memory");
        assert!(total > 0.0);
        let used = *s.memory.used_bytes.value().expect("used memory");
        assert!(used <= total);
        let cores = s.cpu.per_core_percent.value().expect("per-core");
        assert!(!cores.is_empty());
        assert!(cores.iter().all(|v| (0.0..=100.0).contains(v)));
        if let Some(u) = s.cpu.usage_percent.value() {
            assert!((0.0..=100.0).contains(u));
        }
        for g in &s.gpus {
            assert!(g.id.starts_with("luid:"));
            if let Some(u) = g.usage_percent.value() {
                assert!((0.0..=100.0).contains(u));
            }
        }
        if cfg!(windows) {
            assert!(!s.cpu.temperature_c.is_available());
            assert!(s.cpu.max_mhz.value().is_some_and(|v| *v > 0.0));
            assert!(
                s.memory
                    .commit_limit_bytes
                    .value()
                    .is_some_and(|v| *v > 0.0)
            );
            if let Some(a) = s.storage.active_time_percent.value() {
                assert!((0.0..=100.0).contains(a));
            }
        }
    }
}
