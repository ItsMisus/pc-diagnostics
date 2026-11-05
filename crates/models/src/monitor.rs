//! Live monitoring contracts.
//!
//! Every live value is `Metric<f64>`: one numeric type keeps the TS side simple and is
//! exact for byte counts below 2^53.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::metric::Metric;

/// Tauri event carrying each new `LiveSample`.
pub const LIVE_SAMPLE_EVENT: &str = "monitor:sample";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CpuSample {
    /// Whole-system utilization, 0–100 across all logical processors.
    pub usage_percent: Metric<f64>,
    pub per_core_percent: Metric<Vec<f64>>,
    /// Average current frequency across logical processors.
    pub current_mhz: Metric<f64>,
    pub max_mhz: Metric<f64>,
    /// OS-reported frequency limit. Below `max_mhz` signals a limit, not its cause.
    pub limit_mhz: Metric<f64>,
    pub temperature_c: Metric<f64>,
    pub power_w: Metric<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemorySample {
    pub total_bytes: Metric<f64>,
    pub used_bytes: Metric<f64>,
    pub available_bytes: Metric<f64>,
    /// Committed virtual memory and the commit limit (RAM + page file).
    pub commit_used_bytes: Metric<f64>,
    pub commit_limit_bytes: Metric<f64>,
    /// Hard page faults resolved from disk (`\Memory\Pages/sec`).
    pub pages_per_sec: Metric<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GpuSample {
    /// Matches `GpuInfo::id`.
    pub id: String,
    /// Busiest engine utilization (Task Manager convention), 0–100.
    pub usage_percent: Metric<f64>,
    pub dedicated_used_bytes: Metric<f64>,
    pub temperature_c: Metric<f64>,
    pub core_clock_mhz: Metric<f64>,
    pub power_w: Metric<f64>,
    /// Board power as a share of the adapter's maximum, as the driver reports it (D3DKMT).
    /// Not convertible to watts: the reference maximum is not exposed.
    pub power_percent: Metric<f64>,
    /// Fan speed. `0` is a real reading (fan stopped at idle) when the adapter has a fan.
    pub fan_rpm: Metric<f64>,
}

/// Aggregate physical disk activity (`\PhysicalDisk(_Total)`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StorageSample {
    pub read_bytes_per_sec: Metric<f64>,
    pub write_bytes_per_sec: Metric<f64>,
    pub active_time_percent: Metric<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LiveSample {
    /// Monotonic sequence number since app start. Lets the UI drop duplicates/out-of-order.
    #[ts(type = "number")]
    pub seq: u64,
    #[ts(type = "number")]
    pub at: u64,
    pub cpu: CpuSample,
    pub memory: MemorySample,
    pub gpus: Vec<GpuSample>,
    pub storage: StorageSample,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResourceUsage {
    /// CPU share of the whole machine, 0–100 (normalized by logical processor count).
    pub cpu_percent: Metric<f64>,
    pub working_set_bytes: Metric<f64>,
    pub process_count: u32,
}

/// The app's own footprint: Rust core process plus its WebView2 child processes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppOverhead {
    pub core: ResourceUsage,
    pub webview: ResourceUsage,
    #[ts(type = "number")]
    pub window_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorStatus {
    pub interval_ms: u32,
    pub history_capacity: u32,
    #[ts(type = "number")]
    pub samples_collected: u64,
}
