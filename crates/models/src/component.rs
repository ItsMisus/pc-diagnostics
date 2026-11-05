use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Subsystem a metric, benchmark or finding refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ComponentKind {
    Cpu,
    Gpu,
    Memory,
    Storage,
    Drivers,
    Processes,
    System,
}
