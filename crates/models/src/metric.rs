use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;
use crate::time::now_unix_ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Unit {
    /// Non-numeric value (text, enum, list).
    None,
    Percent,
    Bytes,
    BytesPerSecond,
    Megahertz,
    Celsius,
    Watts,
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Count,
    OperationsPerSecond,
    /// Floating-point operations per second (GPU compute throughput).
    FlopsPerSecond,
}

/// Where a value came from. Shown in the UI so every number is traceable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Source {
    Sysinfo,
    Pdh,
    Dxgi,
    Nvml,
    PowerApi,
    Wmi,
    ConfigManager,
    WindowsUpdate,
    Benchmark,
    /// Native kernel query (`NtQuerySystemInformation`).
    Kernel,
    /// WDDM performance data reported by the graphics driver (`D3DKMTQueryAdapterInfo`,
    /// the same source as Task Manager). Vendor-neutral.
    D3dkmt,
    /// macOS I/O Kit registry (GPU statistics, block storage counters, HID sensors).
    IoKit,
    /// macOS Mach kernel statistics (`host_statistics64`, `sysctl`).
    Mach,
}

/// Why a value is missing. There is no "unknown": every gap has an explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export)]
pub enum Unavailability {
    /// Hardware or OS does not expose this value.
    NotSupported,
    /// An optional provider (e.g. `nvml.dll`) is not present. Detail names it.
    ProviderMissing(String),
    /// Reading requires privileges the app does not hold.
    PermissionDenied,
    /// The app does not implement this provider yet.
    NotImplemented,
    /// The provider exists but the read failed.
    Failed(AppError),
}

/// A measured value or an explained absence. Never a fabricated placeholder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export)]
pub enum Metric<T> {
    Available {
        value: T,
        unit: Unit,
        source: Source,
        /// Sample time, Unix epoch milliseconds.
        #[ts(type = "number")]
        at: u64,
    },
    Unavailable {
        reason: Unavailability,
    },
}

impl<T> Metric<T> {
    pub fn available(value: T, unit: Unit, source: Source) -> Self {
        Self::Available {
            value,
            unit,
            source,
            at: now_unix_ms(),
        }
    }

    pub fn unavailable(reason: Unavailability) -> Self {
        Self::Unavailable { reason }
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Available { value, .. } => Some(value),
            Self::Unavailable { .. } => None,
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_serializes_tagged() {
        let m = Metric::Available {
            value: 42.5_f64,
            unit: Unit::Percent,
            source: Source::Pdh,
            at: 1000,
        };
        let json = serde_json::to_value(&m).expect("serialize");
        assert_eq!(json["status"], "available");
        assert_eq!(json["value"], 42.5);
        assert_eq!(json["unit"], "percent");
        assert_eq!(json["source"], "pdh");
        assert_eq!(json["at"], 1000);
    }

    #[test]
    fn unavailable_carries_reason() {
        let m: Metric<f64> =
            Metric::unavailable(Unavailability::ProviderMissing("nvml.dll".into()));
        let json = serde_json::to_value(&m).expect("serialize");
        assert_eq!(json["status"], "unavailable");
        assert_eq!(json["reason"]["kind"], "providerMissing");
        assert_eq!(json["reason"]["detail"], "nvml.dll");
        assert!(m.value().is_none());
    }

    #[test]
    fn unit_variant_reason_has_no_detail() {
        let m: Metric<u64> = Metric::unavailable(Unavailability::PermissionDenied);
        let json = serde_json::to_value(&m).expect("serialize");
        assert_eq!(
            json["reason"],
            serde_json::json!({ "kind": "permissionDenied" })
        );
    }

    #[test]
    fn roundtrips() {
        let m = Metric::available(3_u64, Unit::Count, Source::Sysinfo);
        let s = serde_json::to_string(&m).expect("serialize");
        let back: Metric<u64> = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(back, m);
    }
}
