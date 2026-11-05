//! Device and driver contracts (phase 4). Read-only: nothing here installs or changes drivers.
//!
//! Device health and update status are separate axes on purpose: "working" never means
//! "up to date", and a generic Microsoft driver is not a missing driver.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppError;
use crate::metric::Metric;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DeviceCategory {
    Gpu,
    /// System devices, chipset, PCI bridges, SMBus, firmware.
    Chipset,
    Audio,
    Ethernet,
    Wifi,
    Bluetooth,
    StorageController,
    Disk,
    Usb,
    Input,
    Display,
    Other,
}

/// What the OS reports about the device, independent of driver age.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DeviceHealth {
    /// Started, no problem code reported.
    Working,
    /// Windows reports a Configuration Manager problem code.
    ProblemDetected,
    /// No driver installed or not correctly associated (e.g. CM_PROB_FAILED_INSTALL,
    /// CM_PROB_NOT_CONFIGURED), supported by the problem code.
    DriverMissing,
    /// Disabled by the user or policy (CM_PROB_DISABLED). Not a malfunction.
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateSource {
    WindowsUpdate,
}

/// Result of checking for a newer driver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum UpdateStatus {
    /// No check performed yet (online checks are opt-in).
    NotChecked,
    /// An official source offers an update applicable to this exact device.
    UpdateAvailable {
        source: UpdateSource,
        title: String,
        version: Option<String>,
        /// Driver date declared by the update (ISO 8601 date).
        date: Option<String>,
        /// Matched hardware id.
        hardware_id: String,
    },
    /// Installed version confirmed current by an official source with documented criteria.
    /// Reserved for future vendor catalogs; Windows Update alone cannot confirm this.
    Verified {
        source: UpdateSource,
        detail: String,
    },
    /// The available information cannot establish whether a newer version exists.
    NotVerifiable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DriverInfo {
    pub version: Option<String>,
    /// Declared driver date (ISO 8601 date). Age alone never implies "outdated".
    pub date: Option<String>,
    pub provider: Option<String>,
    pub inf_name: Option<String>,
    pub signed: Metric<bool>,
    pub signer: Option<String>,
    /// Provider is Microsoft (inbox/generic driver). Often correct; not "missing".
    pub is_inbox: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceRecord {
    /// Device instance id. May embed serial numbers: excluded from shareable reports.
    pub instance_id: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub class_name: Option<String>,
    pub class_guid: Option<String>,
    pub category: DeviceCategory,
    pub hardware_ids: Vec<String>,
    pub driver: Option<DriverInfo>,
    pub health: DeviceHealth,
    /// Configuration Manager problem code (`CM_PROB_*`), if any.
    pub problem_code: Option<u32>,
    pub problem_description: Option<String>,
    pub update: UpdateStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DriverSummary {
    pub total: u32,
    pub working: u32,
    pub problems: u32,
    pub driver_missing: u32,
    pub disabled: u32,
    pub updates_available: u32,
    pub not_verifiable: u32,
    pub not_checked: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateCheckInfo {
    #[ts(type = "number")]
    pub checked_at: u64,
    pub source: UpdateSource,
    #[ts(type = "number")]
    pub duration_ms: u64,
    /// Number of driver updates the source offered for this machine.
    pub offered: u32,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DriverInventory {
    #[ts(type = "number")]
    pub collected_at: u64,
    /// Only present (connected) devices are listed.
    pub devices: Vec<DeviceRecord>,
    pub summary: DriverSummary,
    pub update_check: Option<UpdateCheckInfo>,
}

impl DriverSummary {
    pub fn from_devices(devices: &[DeviceRecord]) -> Self {
        let mut s = Self {
            total: u32::try_from(devices.len()).unwrap_or(u32::MAX),
            working: 0,
            problems: 0,
            driver_missing: 0,
            disabled: 0,
            updates_available: 0,
            not_verifiable: 0,
            not_checked: 0,
        };
        for d in devices {
            match d.health {
                DeviceHealth::Working => s.working += 1,
                DeviceHealth::ProblemDetected => s.problems += 1,
                DeviceHealth::DriverMissing => s.driver_missing += 1,
                DeviceHealth::Disabled => s.disabled += 1,
            }
            match d.update {
                UpdateStatus::UpdateAvailable { .. } => s.updates_available += 1,
                UpdateStatus::NotVerifiable { .. } => s.not_verifiable += 1,
                UpdateStatus::NotChecked => s.not_checked += 1,
                UpdateStatus::Verified { .. } => {}
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(health: DeviceHealth, update: UpdateStatus) -> DeviceRecord {
        DeviceRecord {
            instance_id: "X".into(),
            name: "d".into(),
            manufacturer: None,
            class_name: None,
            class_guid: None,
            category: DeviceCategory::Other,
            hardware_ids: vec![],
            driver: None,
            health,
            problem_code: None,
            problem_description: None,
            update,
        }
    }

    #[test]
    fn summary_counts_each_axis_independently() {
        let devices = vec![
            device(DeviceHealth::Working, UpdateStatus::NotChecked),
            device(
                DeviceHealth::Working,
                UpdateStatus::NotVerifiable { reason: "r".into() },
            ),
            device(
                DeviceHealth::ProblemDetected,
                UpdateStatus::UpdateAvailable {
                    source: UpdateSource::WindowsUpdate,
                    title: "t".into(),
                    version: None,
                    date: None,
                    hardware_id: "h".into(),
                },
            ),
            device(DeviceHealth::Disabled, UpdateStatus::NotChecked),
        ];
        let s = DriverSummary::from_devices(&devices);
        assert_eq!((s.total, s.working, s.problems, s.disabled), (4, 2, 1, 1));
        assert_eq!(
            (s.updates_available, s.not_verifiable, s.not_checked),
            (1, 1, 2)
        );
    }

    #[test]
    fn update_status_serializes_with_kind_tag() {
        let json = serde_json::to_value(UpdateStatus::NotVerifiable { reason: "x".into() })
            .expect("serialize");
        assert_eq!(json["kind"], "notVerifiable");
        assert_eq!(json["reason"], "x");
    }
}
