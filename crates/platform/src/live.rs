//! Live (per-tick) collection. Cheap by design: no WMI, no process enumeration, no
//! adapter enumeration inside `sample()`.
//!
//! `LiveCollector` holds raw OS handles and is meant to live on a single sampler thread.
//!
//! macOS (`macos::live`): CPU usage and RAM from `sysinfo`; CPU/SoC temperature from the
//! I/O Kit HID sensors; rated maximum frequency from the power manager's DVFS table;
//! page-ins from Mach `host_statistics64`; GPU utilization and disk throughput from
//! I/O Kit registry statistics. What macOS does not expose without root (current CPU
//! clock, CPU/GPU power, commit charge, disk active time) stays unavailable with a reason.

use models::metric::{Metric, Source, Unavailability, Unit};
use models::monitor::{CpuSample, LiveSample, MemorySample, StorageSample};
use models::time::now_unix_ms;
use models::{AppError, ErrorCode};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

/// Reason for CPU temperature and package power on Windows.
#[cfg(windows)]
pub const CPU_SENSOR_PROVIDER: &str =
    "CPU sensor provider (requires a kernel-mode driver; not bundled)";

pub struct LiveCollector {
    sys: Result<System, Unavailability>,
    #[cfg(windows)]
    win: crate::windows::live::WinLive,
    #[cfg(target_os = "macos")]
    mac: crate::macos::live::MacLive,
}

#[cfg(windows)]
fn cpu_sensor_reason() -> Unavailability {
    Unavailability::ProviderMissing(CPU_SENSOR_PROVIDER.into())
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
    /// on the sampler thread, not on a latency-sensitive one. On macOS it enumerates the
    /// thermal sensors, GPUs and disks once.
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
            #[cfg(target_os = "macos")]
            mac: crate::macos::live::MacLive::new(),
        }
    }

    /// Collects one sample. Every field is either measured or carries its reason.
    pub fn sample(&mut self, seq: u64) -> LiveSample {
        let at = now_unix_ms();
        #[cfg(windows)]
        self.win.collect();
        #[cfg(target_os = "macos")]
        self.mac.collect();
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
        #[cfg(windows)]
        let (temperature_c, power_w) = (
            Metric::unavailable(cpu_sensor_reason()),
            Metric::unavailable(cpu_sensor_reason()),
        );
        #[cfg(target_os = "macos")]
        let (current_mhz, max_mhz, limit_mhz) = self.mac.cpu_frequency();
        #[cfg(target_os = "macos")]
        let (temperature_c, power_w) = (self.mac.cpu_temperature(), self.mac.cpu_power());
        #[cfg(not(any(windows, target_os = "macos")))]
        let (current_mhz, max_mhz, limit_mhz, temperature_c, power_w) = (
            Metric::unavailable(Unavailability::NotSupported),
            Metric::unavailable(Unavailability::NotSupported),
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
            temperature_c,
            power_w,
        }
    }

    /// RAM totals. Windows and other systems: used = total − available (`sysinfo`).
    ///
    /// macOS: used is Activity Monitor's "Memory Used" (`sysinfo` `used_memory`: app
    /// memory + wired + compressed, from `host_statistics64`) and available is
    /// total − used, i.e. cached files + free: what the system can hand out without
    /// compressing or swapping app memory, the meaning of Windows "Available" (standby +
    /// free). `sysinfo`'s own macOS `available_memory` (active + inactive + free) also
    /// counts pages apps are using, so it is not used here.
    fn memory(&mut self) -> MemorySample {
        let (total_bytes, used_bytes, available_bytes) = match &mut self.sys {
            Ok(sys) => {
                sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
                let total = sys.total_memory();
                #[cfg(target_os = "macos")]
                let avail = total.saturating_sub(sys.used_memory());
                #[cfg(not(target_os = "macos"))]
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
        #[cfg(target_os = "macos")]
        let (commit_used_bytes, commit_limit_bytes, pages_per_sec) = self.mac.memory_paging();
        #[cfg(not(any(windows, target_os = "macos")))]
        let (commit_used_bytes, commit_limit_bytes, pages_per_sec) = (
            Metric::unavailable(Unavailability::NotSupported),
            Metric::unavailable(Unavailability::NotSupported),
            Metric::unavailable(Unavailability::NotSupported),
        );
        #[cfg(target_os = "macos")]
        let (swap_used_bytes, swap_total_bytes, pressure) = self.mac.memory_swap_pressure();
        // Windows: the page file is part of the commit limit and there is no kernel
        // pressure level comparable to macOS'.
        #[cfg(not(target_os = "macos"))]
        let (swap_used_bytes, swap_total_bytes, pressure) = (
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
            swap_used_bytes,
            swap_total_bytes,
            pressure,
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(clippy::needless_pass_by_ref_mut))]
    fn gpus(&mut self) -> Vec<models::monitor::GpuSample> {
        #[cfg(windows)]
        return self.win.gpus();
        #[cfg(target_os = "macos")]
        return self.mac.gpus();
        #[cfg(not(any(windows, target_os = "macos")))]
        return Vec::new();
    }

    #[cfg_attr(not(target_os = "macos"), allow(clippy::needless_pass_by_ref_mut))]
    fn storage(&mut self) -> StorageSample {
        #[cfg(windows)]
        return self.win.storage();
        #[cfg(target_os = "macos")]
        return self.mac.storage();
        #[cfg(not(any(windows, target_os = "macos")))]
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
            let prefix = if cfg!(target_os = "macos") {
                "ioreg:"
            } else {
                "luid:"
            };
            assert!(g.id.starts_with(prefix), "{}", g.id);
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

    #[cfg(target_os = "macos")]
    fn reason<T>(m: &Metric<T>) -> Option<&Unavailability> {
        match m {
            Metric::Unavailable { reason } => Some(reason),
            Metric::Available { .. } => None,
        }
    }

    /// Physical RAM as the kernel reports it (`hw.memsize`), independent of `sysinfo`.
    #[cfg(target_os = "macos")]
    fn hw_memsize() -> u64 {
        let mut v: u64 = 0;
        let mut len = std::mem::size_of::<u64>();
        // SAFETY: NUL-terminated name; `v`/`len` describe a writable 8-byte buffer.
        let r = unsafe {
            libc::sysctlbyname(
                c"hw.memsize".as_ptr(),
                (&raw mut v).cast(),
                &raw mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        assert_eq!(r, 0, "sysctl hw.memsize");
        v
    }

    // Two samples ~1 s apart on this Mac: every value macOS exposes must be measured and
    // plausible; everything else must carry its reason.
    #[cfg(target_os = "macos")]
    #[test]
    fn host_macos_samples_are_plausible() {
        let mut c = LiveCollector::new();
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let a = c.sample(1);
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let b = c.sample(2);
        #[allow(clippy::cast_precision_loss)]
        let ram = hw_memsize() as f64;
        for s in [&a, &b] {
            let u = *s.cpu.usage_percent.value().expect("cpu usage");
            assert!((0.0..=100.0).contains(&u));
            assert_eq!(*s.memory.total_bytes.value().expect("total"), ram);
            let used = *s.memory.used_bytes.value().expect("used");
            let avail = *s.memory.available_bytes.value().expect("available");
            assert!(used > 0.0 && avail > 0.0 && used + avail <= ram + 1.0);
            let pages = *s.memory.pages_per_sec.value().expect("pageins/s");
            assert!(pages.is_finite() && pages >= 0.0);
            let swap_used = *s.memory.swap_used_bytes.value().expect("swap used");
            let swap_total = *s.memory.swap_total_bytes.value().expect("swap total");
            assert!(swap_used >= 0.0 && swap_used <= swap_total);
            assert!(s.memory.pressure.is_available(), "memory pressure");
            assert_eq!(
                reason(&s.memory.commit_used_bytes),
                Some(&Unavailability::NotSupported)
            );
            assert_eq!(
                reason(&s.memory.commit_limit_bytes),
                Some(&Unavailability::NotSupported)
            );
            assert_eq!(
                reason(&s.cpu.current_mhz),
                Some(&Unavailability::NotSupported)
            );
            assert_eq!(
                reason(&s.cpu.limit_mhz),
                Some(&Unavailability::NotSupported)
            );
            assert_eq!(
                reason(&s.cpu.power_w),
                Some(&Unavailability::PermissionDenied)
            );
            let r = *s.storage.read_bytes_per_sec.value().expect("disk read/s");
            let w = *s.storage.write_bytes_per_sec.value().expect("disk write/s");
            assert!(r >= 0.0 && w >= 0.0);
            assert!(!s.storage.active_time_percent.is_available());
            assert!(!s.gpus.is_empty(), "the GPU is listed");
            if cfg!(target_arch = "aarch64") {
                let t = *s.cpu.temperature_c.value().expect("SoC die temperature");
                assert!((10.0..=110.0).contains(&t), "{t} °C");
                let m = *s.cpu.max_mhz.value().expect("max MHz");
                assert!((2000.0..=6000.0).contains(&m));
                let g = &s.gpus[0];
                let gu = *g.usage_percent.value().expect("GPU utilization");
                assert!((0.0..=100.0).contains(&gu));
                assert!(!g.dedicated_used_bytes.is_available(), "unified memory");
            }
        }
        assert_eq!(a.gpus[0].id, b.gpus[0].id, "stable GPU id");
    }
}
