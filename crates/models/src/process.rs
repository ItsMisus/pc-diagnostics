//! Process analysis contracts (phase 3).
//!
//! The Impact Score estimates *pressure on resources*. It is not evidence that a process
//! is slowing the computer down. Methodology: docs/impact-score.md.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::metric::Metric;

/// Tauri event carrying each new `ProcessSnapshot`.
pub const PROCESS_SNAPSHOT_EVENT: &str = "processes:snapshot";

/// Bumped whenever the score formula or normalization changes. Scores are comparable
/// only within the same version.
pub const IMPACT_METHODOLOGY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImpactWeights {
    pub cpu: f64,
    pub ram: f64,
    pub io: f64,
}

impl ImpactWeights {
    pub const DEFAULT: Self = Self {
        cpu: 0.45,
        ram: 0.25,
        io: 0.30,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProcessCategory {
    /// Kernel and core OS processes (e.g. System, Registry, Memory Compression).
    System,
    /// Runs in session 0 (services and their hosts).
    Service,
    /// Interactive session with a visible top-level window.
    Application,
    /// Interactive session without a visible window.
    Background,
    Unknown,
}

/// Per-resource pressure, each normalized to 0–100, plus the weighted score.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Pressure {
    pub cpu: f64,
    pub ram: f64,
    pub io: f64,
    pub impact: f64,
}

/// One process in a ranking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessRow {
    /// Stable instance id: `<pid>:<creation time in ms>`. Survives PID reuse.
    pub key: String,
    pub pid: u32,
    pub name: String,
    pub category: ProcessCategory,
    /// False when the process cannot be opened without elevation (protected/system process):
    /// its executable path is unavailable. Its metrics still come from the kernel snapshot.
    pub accessible: bool,
    /// This app's own process or one of its WebView2 children.
    pub is_self: bool,
    /// Full executable path. Personal data: never included in shareable reports.
    pub exe_path: Option<String>,
    /// Share of the whole machine, 0–100 (normalized by logical processor count).
    pub cpu_percent: Metric<f64>,
    pub working_set_bytes: Metric<f64>,
    pub private_bytes: Metric<f64>,
    /// Logical I/O issued by the process (files, network, devices). Not physical disk bytes.
    pub io_read_bytes_per_sec: Metric<f64>,
    pub io_write_bytes_per_sec: Metric<f64>,
    pub pressure: Pressure,
    /// Highest impact observed in the analysis window (instant rows: equals `pressure.impact`).
    pub impact_peak: f64,
}

/// System context used for normalization, reported so the UI can explain scores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessContext {
    pub logical_processors: u32,
    pub total_memory_bytes: f64,
    pub available_memory_bytes: f64,
    /// Reference logical I/O rate that maps to I/O pressure 100.
    pub io_reference_bytes_per_sec: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessSnapshot {
    #[ts(type = "number")]
    pub at: u64,
    pub interval_ms: u32,
    /// Length of the window used for `ranking_window`, in seconds (may be shorter right after start).
    pub window_s: u32,
    pub methodology_version: u32,
    pub weights: ImpactWeights,
    pub context: ProcessContext,
    pub total_processes: u32,
    pub inaccessible_processes: u32,
    /// Top N by impact on the latest sample.
    pub ranking_instant: Vec<ProcessRow>,
    /// Top N by mean impact over the window; `impact_peak` is the window maximum.
    pub ranking_window: Vec<ProcessRow>,
}

/// One point of a process's recent history (detail view).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessPoint {
    #[ts(type = "number")]
    pub at: u64,
    pub cpu_percent: Option<f64>,
    pub working_set_bytes: Option<f64>,
    pub io_read_bytes_per_sec: Option<f64>,
    pub io_write_bytes_per_sec: Option<f64>,
    pub impact: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessHistory {
    pub key: String,
    pub name: String,
    pub points: Vec<ProcessPoint>,
    /// False once the process has exited; history is kept briefly afterwards.
    pub running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_weights_sum_to_one() {
        let w = ImpactWeights::DEFAULT;
        assert!((w.cpu + w.ram + w.io - 1.0).abs() < 1e-12);
    }
}
