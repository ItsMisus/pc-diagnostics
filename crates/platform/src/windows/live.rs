//! Windows live providers: PDH counters, processor power information, DXGI adapter list
//! (enumerated once) and optional NVML.

use std::collections::HashMap;

use models::metric::{Metric, Source, Unavailability, Unit};
use models::monitor::{GpuSample, StorageSample};

use super::dxgi::{self, Adapter};
use super::kmt::{KmtAdapter, KmtCaps, KmtPerf};
use super::nvml::{self, NVML_DLL, NvmlState};
use super::pdh::{Counter, Query, RawValue};
use super::power;
use crate::convert::{
    Luid, active_from_idle, aggregate_engine_utilization, average_nonzero, busy_percent_from_raw,
    clamp_percent, gpu_id, split_processor_items, sum_per_luid,
};

const NVIDIA_PCI_VENDOR: u32 = 0x10DE;

struct PdhSet {
    query: Query,
    cpu_time: Result<Counter, Unavailability>,
    cpu_performance: Result<Counter, Unavailability>,
    pages_per_sec: Result<Counter, Unavailability>,
    commit_used: Result<Counter, Unavailability>,
    commit_limit: Result<Counter, Unavailability>,
    disk_read: Result<Counter, Unavailability>,
    disk_write: Result<Counter, Unavailability>,
    disk_idle: Result<Counter, Unavailability>,
    gpu_engine: Result<Counter, Unavailability>,
    gpu_dedicated: Result<Counter, Unavailability>,
}

impl PdhSet {
    fn open() -> Result<Self, Unavailability> {
        let query = Query::open().map_err(Unavailability::Failed)?;
        let add = |path: &str| {
            let r = query.add(path);
            if let Err(e) = &r {
                tracing::warn!(path, reason = ?e, "PDH counter unavailable");
            }
            r
        };
        let set = Self {
            cpu_time: add(r"\Processor(*)\% Processor Time"),
            cpu_performance: add(r"\Processor Information(_Total)\% Processor Performance"),
            pages_per_sec: add(r"\Memory\Pages/sec"),
            commit_used: add(r"\Memory\Committed Bytes"),
            commit_limit: add(r"\Memory\Commit Limit"),
            disk_read: add(r"\PhysicalDisk(_Total)\Disk Read Bytes/sec"),
            disk_write: add(r"\PhysicalDisk(_Total)\Disk Write Bytes/sec"),
            disk_idle: add(r"\PhysicalDisk(_Total)\% Idle Time"),
            gpu_engine: add(r"\GPU Engine(*)\Utilization Percentage"),
            gpu_dedicated: add(r"\GPU Adapter Memory(*)\Dedicated Usage"),
            query,
        };
        Ok(set)
    }
}

/// Per-sample GPU counter values, read once and shared by all adapters.
struct GpuCounters {
    usage: Result<HashMap<Luid, f64>, Unavailability>,
    dedicated: Result<HashMap<Luid, f64>, Unavailability>,
}

pub struct WinLive {
    pdh: Result<PdhSet, Unavailability>,
    /// Result of the last `PdhCollectQueryData`.
    collected: Result<(), Unavailability>,
    adapters: Result<Vec<Adapter>, Unavailability>,
    nvml: NvmlState,
    /// WDDM perf-data handles per physical adapter (vendor-neutral sensors).
    kmt: HashMap<Luid, Result<(KmtAdapter, KmtCaps), models::AppError>>,
    /// Previous raw `% Processor Time` reading per instance.
    prev_cpu_raw: HashMap<String, RawValue>,
}

fn read(
    counter: &Result<Counter, Unavailability>,
    collected: &Result<(), Unavailability>,
) -> Result<f64, Unavailability> {
    collected.clone()?;
    let c = counter.as_ref().map_err(Clone::clone)?;
    c.value().map_err(Unavailability::Failed)
}

fn to_metric(r: Result<Option<f64>, Unavailability>, unit: Unit, source: Source) -> Metric<f64> {
    match r {
        Ok(Some(v)) => Metric::available(v, unit, source),
        Ok(None) => Metric::unavailable(Unavailability::Failed(models::AppError::new(
            models::ErrorCode::Platform,
            "counter returned a non-finite value",
        ))),
        Err(reason) => Metric::unavailable(reason),
    }
}

fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(v.max(0.0))
}

impl WinLive {
    /// Opens all providers and performs the first PDH collection, so rate counters have a
    /// baseline and the first real sample carries data.
    pub fn new() -> Self {
        let pdh = PdhSet::open();
        let collected = match &pdh {
            Ok(p) => p.query.collect().map_err(Unavailability::Failed),
            Err(e) => Err(e.clone()),
        };
        let adapters = dxgi::enumerate_adapters().map_err(|e| {
            tracing::warn!(error = %e, "DXGI enumeration failed");
            Unavailability::Failed(e)
        });
        let nvml = match &adapters {
            Ok(a) => nvml::init(a),
            Err(_) => NvmlState::Missing,
        };
        if let NvmlState::Failed(e) = &nvml {
            tracing::warn!(error = %e, "NVML init failed");
        }
        let kmt = match &adapters {
            Ok(list) => list
                .iter()
                .filter(|a| !a.is_software && !a.kmt.is_indirect_display())
                .map(|a| {
                    let opened = KmtAdapter::open(a.luid).map(|k| {
                        let caps = k.caps();
                        (k, caps)
                    });
                    if let Err(e) = &opened {
                        tracing::warn!(name = a.name, error = %e, "D3DKMT adapter open failed");
                    }
                    (a.luid, opened)
                })
                .collect(),
            Err(_) => HashMap::new(),
        };
        let mut live = Self {
            pdh,
            collected,
            adapters,
            nvml,
            kmt,
            prev_cpu_raw: HashMap::new(),
        };
        // Baseline raw CPU reading, so the first sample has a delta.
        let _ = live.cpu_usage();
        live
    }

    /// One PDH collection for this sample. Call before reading counters.
    pub fn collect(&mut self) {
        self.collected = match &self.pdh {
            Ok(p) => p.query.collect().map_err(|e| {
                tracing::warn!(error = %e, "PDH collect failed");
                Unavailability::Failed(e)
            }),
            Err(e) => Err(e.clone()),
        };
    }

    fn counter(
        &self,
        f: impl Fn(&PdhSet) -> &Result<Counter, Unavailability>,
    ) -> Result<f64, Unavailability> {
        let p = self.pdh.as_ref().map_err(Clone::clone)?;
        read(f(p), &self.collected)
    }

    /// `(total, per logical processor)` busy time from `\Processor(*)\% Processor Time`.
    ///
    /// Computed from raw values (`100 × (1 − Δidle / Δtime)`), not PDH's formatted value:
    /// Windows accounts idle time in scheduler-tick quanta, so on a mostly idle core Δidle
    /// can exceed Δtime and PDH then flags the instance `PDH_CSTATUS_INVALID_DATA`. Such a
    /// window was fully idle, and the result is clamped to 0 % instead of being dropped.
    pub fn cpu_usage(&mut self) -> (Metric<f64>, Metric<Vec<f64>>) {
        let raw = (|| {
            let p = self.pdh.as_ref().map_err(Clone::clone)?;
            self.collected.clone()?;
            let c = p.cpu_time.as_ref().map_err(Clone::clone)?;
            c.raw_array().map_err(Unavailability::Failed)
        })();
        let raw = match raw {
            Ok(r) => r,
            Err(r) => return (Metric::unavailable(r.clone()), Metric::unavailable(r)),
        };
        let items: Vec<(String, Option<f64>)> = raw
            .iter()
            .map(|(name, cur)| {
                let v = self.prev_cpu_raw.get(name).and_then(|prev| {
                    busy_percent_from_raw((prev.first, prev.second), (cur.first, cur.second))
                });
                (name.clone(), v)
            })
            .collect();
        self.prev_cpu_raw = raw.into_iter().collect();
        let (total, cores) = split_processor_items(&items);
        let bad = |what: &str| {
            Unavailability::Failed(models::AppError::new(
                models::ErrorCode::Platform,
                format!("{what}: no previous raw reading yet or counter reset"),
            ))
        };
        (
            match total {
                Some(v) => Metric::available(v, Unit::Percent, Source::Pdh),
                None => Metric::unavailable(bad("CPU total")),
            },
            match cores {
                Some(v) => Metric::available(v, Unit::Percent, Source::Pdh),
                None => Metric::unavailable(bad("per-core CPU")),
            },
        )
    }

    /// `(current, max, limit)` MHz.
    ///
    /// `max` and `limit` are `CallNtPowerInformation` averages across logical processors.
    /// `current` is the effective frequency the way Task Manager computes it:
    /// `% Processor Performance` (PDH, can exceed 100 under boost) × average `MaxMhz`.
    /// `PROCESSOR_POWER_INFORMATION.CurrentMhz` is not used: on current Windows builds it
    /// stays pinned at `MaxMhz` and does not track the real clock.
    pub fn cpu_frequency(&self) -> (Metric<f64>, Metric<f64>, Metric<f64>) {
        let info = match power::processor_power_info() {
            Ok(info) => info,
            Err(e) => {
                tracing::warn!(error = %e, "processor power information failed");
                let r = Unavailability::Failed(e);
                return (
                    Metric::unavailable(r.clone()),
                    Metric::unavailable(r.clone()),
                    Metric::unavailable(r),
                );
            }
        };
        let m = |v: Option<f64>| match v {
            Some(v) => Metric::available(v, Unit::Megahertz, Source::PowerApi),
            None => Metric::unavailable(Unavailability::NotSupported),
        };
        let max = average_nonzero(info.iter().map(|p| p.MaxMhz));
        let limit = average_nonzero(info.iter().map(|p| p.MhzLimit));
        let current = match (self.counter(|p| &p.cpu_performance), max) {
            (Ok(perf), Some(max)) if perf.is_finite() && perf >= 0.0 => {
                Metric::available(perf * max / 100.0, Unit::Megahertz, Source::Pdh)
            }
            (Ok(_), Some(_)) => Metric::unavailable(Unavailability::Failed(models::AppError::new(
                models::ErrorCode::Platform,
                "% Processor Performance returned a non-finite value",
            ))),
            (Err(r), _) => Metric::unavailable(r),
            (_, None) => Metric::unavailable(Unavailability::NotSupported),
        };
        (current, m(max), m(limit))
    }

    /// `(commit used, commit limit, pages/sec)`.
    pub fn memory_paging(&self) -> (Metric<f64>, Metric<f64>, Metric<f64>) {
        (
            to_metric(
                self.counter(|p| &p.commit_used).map(finite),
                Unit::Bytes,
                Source::Pdh,
            ),
            to_metric(
                self.counter(|p| &p.commit_limit).map(finite),
                Unit::Bytes,
                Source::Pdh,
            ),
            to_metric(
                self.counter(|p| &p.pages_per_sec).map(finite),
                Unit::OperationsPerSecond,
                Source::Pdh,
            ),
        )
    }

    pub fn storage(&self) -> StorageSample {
        StorageSample {
            read_bytes_per_sec: to_metric(
                self.counter(|p| &p.disk_read).map(finite),
                Unit::BytesPerSecond,
                Source::Pdh,
            ),
            write_bytes_per_sec: to_metric(
                self.counter(|p| &p.disk_write).map(finite),
                Unit::BytesPerSecond,
                Source::Pdh,
            ),
            active_time_percent: to_metric(
                self.counter(|p| &p.disk_idle).map(active_from_idle),
                Unit::Percent,
                Source::Pdh,
            ),
        }
    }

    fn gpu_counters(&self) -> GpuCounters {
        let array = |f: &dyn Fn(&PdhSet) -> &Result<Counter, Unavailability>| {
            let p = self.pdh.as_ref().map_err(Clone::clone)?;
            self.collected.clone()?;
            let c = f(p).as_ref().map_err(Clone::clone)?;
            c.array().map_err(Unavailability::Failed)
        };
        // Engine instances without valid data this round (processes that just appeared)
        // are skipped; they carry no utilization yet.
        let valid = |items: &[(String, Option<f64>)]| -> Vec<(String, f64)> {
            items
                .iter()
                .filter_map(|(n, v)| Some((n.clone(), (*v)?)))
                .collect()
        };
        let usage = array(&|p| &p.gpu_engine).map(|items| {
            let items = valid(&items);
            aggregate_engine_utilization(items.iter().map(|(n, v)| (n.as_str(), *v)))
        });
        let dedicated = array(&|p| &p.gpu_dedicated).map(|items| {
            let items = valid(&items);
            sum_per_luid(items.iter().map(|(n, v)| (n.as_str(), *v)))
        });
        GpuCounters { usage, dedicated }
    }

    pub fn gpus(&self) -> Vec<GpuSample> {
        let Ok(adapters) = &self.adapters else {
            return Vec::new();
        };
        let counters = self.gpu_counters();
        adapters
            .iter()
            .map(|a| self.gpu_sample(a, &counters))
            .collect()
    }

    fn gpu_sample(&self, a: &Adapter, c: &GpuCounters) -> GpuSample {
        // An adapter known to PDH (it has a `GPU Adapter Memory` instance) with no engine
        // instances has no client processes, so its utilization is genuinely 0 %.
        let known_to_pdh = |luid: &Luid| {
            c.dedicated.as_ref().is_ok_and(|m| m.contains_key(luid))
                || c.usage.as_ref().is_ok_and(|m| m.contains_key(luid))
        };
        let usage_percent = match &c.usage {
            Ok(m) => match m.get(&a.luid) {
                Some(v) => to_metric(Ok(clamp_percent(*v)), Unit::Percent, Source::Pdh),
                None if known_to_pdh(&a.luid) => Metric::available(0.0, Unit::Percent, Source::Pdh),
                None => Metric::unavailable(Unavailability::NotSupported),
            },
            Err(r) => Metric::unavailable(r.clone()),
        };
        let dedicated_used_bytes = match &c.dedicated {
            Ok(m) => match m.get(&a.luid) {
                Some(v) => Metric::available(*v, Unit::Bytes, Source::Pdh),
                None => Metric::unavailable(Unavailability::NotSupported),
            },
            Err(r) => Metric::unavailable(r.clone()),
        };
        let virtual_adapter = a.is_software || a.kmt.is_indirect_display();
        if virtual_adapter {
            // Software rasterizers and indirect-display (virtual display) adapters have no
            // sensors of their own.
            let na = || Metric::unavailable(Unavailability::NotSupported);
            return GpuSample {
                id: gpu_id(a.luid),
                usage_percent,
                dedicated_used_bytes,
                temperature_c: na(),
                core_clock_mhz: na(),
                power_w: na(),
                power_percent: na(),
                fan_rpm: na(),
            };
        }
        let wddm = self.wddm_perf(a.luid);
        let from_wddm = |pick: fn(&KmtPerf) -> Option<f64>, unit: Unit| match &wddm {
            Ok(p) => match pick(p) {
                Some(v) => Metric::available(v, unit, Source::D3dkmt),
                None => Metric::unavailable(Unavailability::NotSupported),
            },
            Err(e) => Metric::unavailable(Unavailability::Failed(e.clone())),
        };
        let power_percent = from_wddm(|p| p.power_percent, Unit::Percent);
        let fan_rpm = from_wddm(|p| p.fan_rpm, Unit::Count);
        let wddm_temperature = from_wddm(|p| p.temperature_c, Unit::Celsius);
        let wddm_clock = from_wddm(|p| p.core_clock_mhz, Unit::Megahertz);
        let (temperature_c, core_clock_mhz, power_w) = if a.vendor_id == NVIDIA_PCI_VENDOR {
            match &self.nvml {
                NvmlState::Ready(p) if p.is_matched(a.luid) => {
                    let r = p.read(a.luid);
                    (r.temperature_c, r.core_clock_mhz, r.power_w)
                }
                // Without NVML the driver's WDDM data still gives temperature and clock;
                // watts need NVML.
                NvmlState::Failed(e) => (
                    wddm_temperature,
                    wddm_clock,
                    Metric::unavailable(Unavailability::Failed(e.clone())),
                ),
                _ => (
                    wddm_temperature,
                    wddm_clock,
                    Metric::unavailable(Unavailability::ProviderMissing(NVML_DLL.into())),
                ),
            }
        } else {
            // AMD/Intel: WDDM data reports power only as a share of the maximum; watts
            // would need the vendor SDKs (ADLX/IGCL), which the app does not implement.
            (
                wddm_temperature,
                wddm_clock,
                Metric::unavailable(Unavailability::NotImplemented),
            )
        };
        GpuSample {
            id: gpu_id(a.luid),
            usage_percent,
            dedicated_used_bytes,
            temperature_c,
            core_clock_mhz,
            power_w,
            power_percent,
            fan_rpm,
        }
    }

    fn wddm_perf(&self, luid: Luid) -> Result<KmtPerf, models::AppError> {
        match self.kmt.get(&luid) {
            Some(Ok((adapter, caps))) => adapter.perf(caps),
            Some(Err(e)) => Err(e.clone()),
            None => Err(models::AppError::new(
                models::ErrorCode::Platform,
                "adapter not opened for WDDM performance data",
            )),
        }
    }
}
