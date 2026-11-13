//! Per-tick macOS providers behind `LiveCollector`. Everything opened here is opened
//! once (service lists, sensor list, Mach host port) and only re-read per tick.

use models::metric::{Metric, Source, Unavailability, Unit};
use models::monitor::{GpuSample, MemoryPressure, StorageSample};
use models::{AppError, ErrorCode};

use super::gpu::GpuLive;
use super::mem::PageIns;
use super::sensors::Sensors;
use super::storage::StorageLive;

pub struct MacLive {
    sensors: Sensors,
    gpus: GpuLive,
    storage: StorageLive,
    pageins: PageIns,
    max_mhz: Metric<f64>,
    /// Temperatures read at the start of this tick: `(cpu, gpu)`.
    temps: (Option<f64>, Option<f64>),
}

impl MacLive {
    /// Opens the providers and takes the baseline for the rate counters.
    pub fn new() -> Self {
        Self {
            sensors: Sensors::new(),
            gpus: GpuLive::new(),
            storage: StorageLive::new(),
            pageins: PageIns::new(),
            max_mhz: super::cpu::max_mhz(),
            temps: (None, None),
        }
    }

    /// Reads the sensors once per tick (CPU and GPU share one refresh).
    pub fn collect(&mut self) {
        self.temps = self.sensors.read();
    }

    /// `(current, max, limit)`: see `macos::cpu` for why only the maximum exists.
    pub fn cpu_frequency(&self) -> (Metric<f64>, Metric<f64>, Metric<f64>) {
        (
            Metric::unavailable(Unavailability::NotSupported),
            self.max_mhz.clone(),
            Metric::unavailable(Unavailability::NotSupported),
        )
    }

    /// CPU temperature (see `macos::sensors` for the labels used). `NotSupported` when
    /// the Mac has no CPU or SoC die sensor; `Failed` when it has one but no plausible
    /// reading came back this tick.
    pub fn cpu_temperature(&self) -> Metric<f64> {
        match self.temps.0 {
            Some(t) => Metric::available(t, Unit::Celsius, Source::IoKit),
            None if !self.sensors.has_cpu_sensors() => {
                Metric::unavailable(Unavailability::NotSupported)
            }
            None => Metric::unavailable(Unavailability::Failed(AppError::new(
                ErrorCode::Platform,
                "CPU temperature sensors returned no plausible reading",
            ))),
        }
    }

    /// CPU package power: on Apple Silicon only `powermetrics` (root) or the private
    /// IOReport framework expose energy counters; Intel Macs need SMC power keys that
    /// are not read here.
    pub fn cpu_power(&self) -> Metric<f64> {
        Metric::unavailable(Unavailability::PermissionDenied)
    }

    /// `(commit used, commit limit, pages/s)`: see `macos::mem`.
    pub fn memory_paging(&mut self) -> (Metric<f64>, Metric<f64>, Metric<f64>) {
        let pages = match self.pageins.rate() {
            Ok(v) => Metric::available(v, Unit::OperationsPerSecond, Source::Mach),
            Err(e) => Metric::unavailable(Unavailability::Failed(e)),
        };
        (
            Metric::unavailable(Unavailability::NotSupported),
            Metric::unavailable(Unavailability::NotSupported),
            pages,
        )
    }

    /// `(swap used, swap total, pressure)`: see `macos::mem`.
    pub fn memory_swap_pressure(&self) -> (Metric<f64>, Metric<f64>, Metric<MemoryPressure>) {
        #[allow(clippy::cast_precision_loss)] // exact below 2^53 bytes
        let (used, total) = match super::mem::swap_usage() {
            Ok((u, t)) => (
                Metric::available(u as f64, Unit::Bytes, Source::Mach),
                Metric::available(t as f64, Unit::Bytes, Source::Mach),
            ),
            Err(e) => (
                Metric::unavailable(Unavailability::Failed(e.clone())),
                Metric::unavailable(Unavailability::Failed(e)),
            ),
        };
        let pressure = match super::mem::pressure_level() {
            Ok(p) => Metric::available(p, Unit::None, Source::Mach),
            Err(e) => Metric::unavailable(Unavailability::Failed(e)),
        };
        (used, total, pressure)
    }

    pub fn gpus(&mut self) -> Vec<GpuSample> {
        self.gpus.samples(self.temps.1)
    }

    pub fn storage(&mut self) -> StorageSample {
        self.storage.sample()
    }
}
