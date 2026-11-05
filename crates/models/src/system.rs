use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Static operating-system facts. `None` means the OS did not report the field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemInfo {
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub os_long_version: Option<String>,
    pub kernel_version: Option<String>,
    /// Personal data: excluded from shareable reports.
    pub hostname: Option<String>,
    /// CPU architecture as reported by the OS (e.g. `x86_64`).
    pub cpu_arch: String,
    /// Architecture this binary was compiled for.
    pub build_arch: String,
    #[ts(type = "number | null")]
    pub boot_time_unix_s: Option<u64>,
    #[ts(type = "number | null")]
    pub uptime_s: Option<u64>,
}
