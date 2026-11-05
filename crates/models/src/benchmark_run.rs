//! Benchmark run contracts (phase 5): requests, capabilities, progress, results, context.
//!
//! Results are comparable only between runs with the same kind, methodology version,
//! build profile, parameters and hardware fingerprint (see `BenchmarkResult::comparable_with`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::benchmark::{BenchmarkKind, BenchmarkState};
use crate::error::AppError;
use crate::metric::{Metric, Unavailability, Unit};

/// Tauri event carrying `BenchmarkProgress` while a run is active (about every 250 ms).
pub const BENCHMARK_PROGRESS_EVENT: &str = "benchmark:progress";
/// Tauri event carrying the final `BenchmarkResult` (completed, cancelled or failed).
pub const BENCHMARK_FINISHED_EVENT: &str = "benchmark:finished";

/// User request. Every field is validated by the backend against `BenchmarkCapability`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BenchmarkRequest {
    pub kind: BenchmarkKind,
    /// Run length for duration-based tests; `None` = capability default.
    pub duration_s: Option<u32>,
    /// Worker threads for multi-threaded tests; `None` = all logical processors.
    pub threads: Option<u32>,
    /// Storage tests: mount point of the volume to test (e.g. `C:\`).
    pub storage_volume: Option<String>,
    /// Storage tests: size of the temporary test file in MiB; `None` = default.
    pub storage_file_mib: Option<u32>,
    /// Storage tests: explicit consent to write a temporary file (required, must be true).
    pub storage_write_consent: bool,
    /// GPU tests: `GpuInfo::id` of the adapter to test; `None` = primary physical GPU.
    /// Adapter ids (LUIDs) change across reboots, so comparisons ignore this field and
    /// match the GPU by name and driver in `BenchmarkContext` instead.
    #[serde(default)]
    pub gpu_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BenchmarkCapability {
    pub kind: BenchmarkKind,
    /// Available on this machine with this build. `Unavailable` carries the reason.
    pub availability: Metric<bool>,
    /// What the workload does, in plain language (Italian UI copy).
    pub description: String,
    /// Metrics the run produces and monitors.
    pub measures: Vec<String>,
    pub methodology_version: u32,
    pub default_duration_s: Option<u32>,
    pub max_duration_s: Option<u32>,
    pub supports_threads: bool,
    pub max_threads: Option<u32>,
    /// Storage: default and maximum temporary file size (MiB).
    pub default_file_mib: Option<u32>,
    pub max_file_mib: Option<u32>,
    /// Upper bound of bytes written to disk by one run, if the test writes.
    #[ts(type = "number | null")]
    pub max_bytes_written: Option<u64>,
    /// GPU tests: adapters that can be selected (physical GPUs only), primary first.
    pub gpu_targets: Vec<BenchmarkGpuTarget>,
}

/// A GPU the GPU benchmark can run on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BenchmarkGpuTarget {
    /// `GpuInfo::id`.
    pub id: String,
    pub name: String,
    /// The target used when the request has no `gpu_id`.
    pub is_default: bool,
    /// Whether a compute adapter (wgpu DX12/Vulkan) matches it; `Unavailable` with reason.
    pub availability: Metric<bool>,
    /// Temperature at which the run stops, from the driver's own thresholds; `None` when
    /// the driver exposes none (the run is then capped at a shorter duration).
    pub stop_temperature_c: Option<f64>,
    /// Longest run allowed on this target (shorter when no temperature stop is in use).
    pub max_duration_s: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PowerSource {
    Ac,
    Battery,
    Unknown,
}

/// Conditions at start. Used to explain results and to flag non-comparable comparisons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BenchmarkContext {
    pub os_version: Option<String>,
    pub cpu_brand: Option<String>,
    pub logical_processors: u32,
    #[ts(type = "number")]
    pub total_memory_bytes: u64,
    pub power_source: PowerSource,
    /// Active power plan name (e.g. "Balanced").
    pub power_plan: Option<String>,
    /// Whole-machine CPU use in the seconds before the start (background load).
    pub background_cpu_percent: Metric<f64>,
    /// Available memory at start.
    pub available_memory_bytes: Metric<f64>,
    /// OS frequency limit vs max at start (a lower limit hints at power/thermal policy).
    pub cpu_limit_mhz: Metric<f64>,
    pub cpu_max_mhz: Metric<f64>,
    /// Storage tests: model of the disk backing the tested volume.
    pub storage_model: Option<String>,
    /// GPU tests: the physical adapter tested and its driver version.
    pub gpu_name: Option<String>,
    pub gpu_driver_version: Option<String>,
}

/// One named result value, e.g. "Throughput multi-core" in ops/s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResultMetric {
    /// Stable id used to match metrics across runs (e.g. `throughput`).
    pub id: String,
    pub label: String,
    pub value: f64,
    pub unit: Unit,
    pub higher_is_better: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SeriesPoint {
    #[ts(type = "number")]
    pub elapsed_ms: u64,
    pub value: f64,
}

/// A time series recorded during the run (e.g. throughput per second, CPU MHz).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BenchmarkSeries {
    pub id: String,
    pub label: String,
    pub unit: Unit,
    pub points: Vec<SeriesPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BenchmarkProgress {
    pub run_id: String,
    pub kind: BenchmarkKind,
    pub state: BenchmarkState,
    /// 0–1. Exact for fixed-size workloads, elapsed/duration for timed ones.
    pub fraction: f64,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
    /// Short phase description (e.g. "Scrittura sequenziale").
    pub phase: String,
    /// Latest interim values (same ids as the final metrics), for live display.
    pub interim: Vec<ResultMetric>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BenchmarkResult {
    pub run_id: String,
    pub kind: BenchmarkKind,
    /// `Completed`, `Cancelled` or `Failed`.
    pub state: BenchmarkState,
    pub methodology_version: u32,
    pub app_version: String,
    pub build_profile: String,
    #[ts(type = "number")]
    pub started_at: u64,
    #[ts(type = "number")]
    pub finished_at: u64,
    #[ts(type = "number")]
    pub duration_ms: u64,
    /// The request as resolved by the backend (defaults applied).
    pub request: BenchmarkRequest,
    pub context: BenchmarkContext,
    /// Empty unless completed (partial runs never produce headline numbers).
    pub metrics: Vec<ResultMetric>,
    pub series: Vec<BenchmarkSeries>,
    /// Facts the user should know when reading the result (e.g. "running on battery").
    pub warnings: Vec<String>,
    /// Neutral information about how the run was performed (e.g. which GPU adapter was
    /// used). Not a warning.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Set when the safety controller stopped the run, with the reason.
    pub safety_stop: Option<String>,
    pub error: Option<AppError>,
}

impl BenchmarkResult {
    /// Runs are comparable only if the measuring method and the measured system match.
    /// Context differences (power, background load) do not block comparison; they are
    /// reported as caveats by the diagnostics engine.
    pub fn comparable_with(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.state == BenchmarkState::Completed
            && other.state == BenchmarkState::Completed
            && self.methodology_version == other.methodology_version
            && self.build_profile == other.build_profile
            && BenchmarkRequest {
                gpu_id: None,
                ..self.request.clone()
            } == BenchmarkRequest {
                gpu_id: None,
                ..other.request.clone()
            }
            && self.context.cpu_brand == other.context.cpu_brand
            && self.context.logical_processors == other.context.logical_processors
            && self.context.total_memory_bytes == other.context.total_memory_bytes
            && self.context.storage_model == other.context.storage_model
            && self.context.gpu_name == other.context.gpu_name
            && self.context.gpu_driver_version == other.context.gpu_driver_version
    }
}

/// Reason a benchmark is not offered (helper for capability builders).
pub fn unavailable_capability(reason: Unavailability) -> Metric<bool> {
    Metric::Unavailable { reason }
}
