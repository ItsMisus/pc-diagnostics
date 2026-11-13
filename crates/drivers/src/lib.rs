//! Read-only device and driver inventory, plus Windows Update driver-update verification.
//!
//! Nothing in this crate installs, removes, rolls back, enables, disables or otherwise
//! modifies a device or a driver; nothing is downloaded or executed; no admin rights are
//! needed. Consent (online checks) and concurrency gating belong to the caller.
//!
//! - [`collect_inventory`]: SetupAPI/CfgMgr32 enumeration (fast) + one WMI
//!   `Win32_PnPSignedDriver` query for signatures (slow, bounded). No network.
//! - [`check_updates`]: Windows Update Agent search; fills `update` and `update_check`.
//!
//! Health and update status are independent: see docs/driver-verification.md.
//!
//! Windows only. On other platforms (macOS) [`inventory_supported`] is `false` and both
//! entry points return [`unsupported_error`] (`NotSupported`) at once; the pure helpers
//! still build and are tested everywhere.

pub mod category;
pub mod problem;
pub mod updates;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use models::drivers::{
    DeviceCategory, DeviceHealth, DeviceRecord, DriverInfo, DriverInventory, DriverSummary,
    UpdateCheckInfo, UpdateSource, UpdateStatus,
};
use models::metric::{Metric, Source, Unavailability, Unit};
use models::settings::Language;
use models::time::now_unix_ms;
use models::{AppError, ErrorCode};
use platform::devices::{DriverUpdateOffer, RawDevice, SignedDriverRow};

use crate::category::{CategoryInput, categorize};

/// Bound for the WMI signature query. It normally takes a few seconds.
pub const SIGNED_DRIVERS_TIMEOUT: Duration = Duration::from_secs(60);
/// Bound for the Windows Update search (network; can legitimately take minutes).
pub const UPDATE_SEARCH_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Message of [`unsupported_error`] (stable: the UI and tests may match on it).
pub const UNSUPPORTED_MESSAGE: &str =
    "the device and driver inventory is only available on Windows";

/// Whether this build can collect the device and driver inventory. Only Windows: the
/// inventory is built on SetupAPI/CfgMgr32, WMI and Windows Update, and macOS has no
/// per-device driver list that an app can read without privileges (its drivers ship with
/// the system and update with it).
pub const fn inventory_supported() -> bool {
    cfg!(windows)
}

/// The error [`collect_inventory`] and [`check_updates`] return where
/// [`inventory_supported`] is `false`: code `NotSupported`, so the UI shows "not available
/// on this system" rather than a failure, and diagnostics report the component as not
/// evaluated.
pub fn unsupported_error() -> AppError {
    AppError::new(ErrorCode::NotSupported, UNSUPPORTED_MESSAGE)
        .with_detail(format!("platform: {}", std::env::consts::OS))
}

/// Enumerates present devices (fast path: SetupAPI/CfgMgr32) and enriches with signature
/// data. No network. `NotSupported` ([`unsupported_error`]) outside Windows.
pub fn collect_inventory() -> Result<DriverInventory, AppError> {
    if !inventory_supported() {
        return Err(unsupported_error());
    }
    // The WMI query is the slow part: run it concurrently with the enumeration.
    let wmi = std::thread::Builder::new()
        .name("drivers-signed".into())
        .spawn(|| platform::devices::signed_drivers(SIGNED_DRIVERS_TIMEOUT));
    let raw = platform::devices::enumerate_present_devices()?;
    let signed =
        match wmi {
            Ok(h) => h
                .join()
                .unwrap_or_else(|_| Err(AppError::internal("WMI signature thread panicked"))),
            Err(e) => Err(AppError::internal("could not spawn WMI signature thread")
                .with_detail(e.to_string())),
        };
    if let Err(e) = &signed {
        tracing::warn!(error = %e, "driver signature data unavailable");
    }
    Ok(build_inventory(raw, &signed))
}

/// Searches Windows Update for applicable driver updates and returns `inventory` with
/// `update` statuses (reasons in `lang`) and `update_check` filled. Network; may take
/// minutes; blocking. Never installs. `NotSupported` ([`unsupported_error`]) outside
/// Windows.
pub fn check_updates(
    inventory: &DriverInventory,
    lang: Language,
) -> Result<DriverInventory, AppError> {
    if !inventory_supported() {
        return Err(unsupported_error());
    }
    let checked_at = now_unix_ms();
    let started = Instant::now();
    let offers = platform::devices::search_driver_updates(UPDATE_SEARCH_TIMEOUT)?;
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let out = apply_update_search(inventory, &offers, checked_at, duration_ms, lang);
    tracing::info!(
        offered = offers.len(),
        matched = updates::matched_count(&out.devices),
        duration_ms,
        "Windows Update driver search done"
    );
    Ok(out)
}

/// Pure part of [`check_updates`]: statuses, summary and `update_check` from the offers.
pub fn apply_update_search(
    inventory: &DriverInventory,
    offers: &[DriverUpdateOffer],
    checked_at: u64,
    duration_ms: u64,
    lang: Language,
) -> DriverInventory {
    let mut out = inventory.clone();
    updates::apply_offers(&mut out.devices, offers, lang);
    out.summary = DriverSummary::from_devices(&out.devices);
    out.update_check = Some(UpdateCheckInfo {
        checked_at,
        source: UpdateSource::WindowsUpdate,
        duration_ms,
        offered: u32::try_from(offers.len()).unwrap_or(u32::MAX),
        error: None,
    });
    out
}

/// Generic Microsoft (inbox) driver: provider is exactly "Microsoft", ignoring case. Such
/// drivers are often the correct ones and are never treated as missing or problematic.
pub fn is_inbox(provider: Option<&str>) -> bool {
    provider.is_some_and(|p| p.trim().eq_ignore_ascii_case("microsoft"))
}

fn signed_metric(row: Option<&SignedDriverRow>, wmi_error: Option<&AppError>) -> Metric<bool> {
    if let Some(e) = wmi_error {
        return Metric::unavailable(if e.code == ErrorCode::PermissionDenied {
            Unavailability::PermissionDenied
        } else {
            Unavailability::Failed(e.clone())
        });
    }
    match row {
        Some(SignedDriverRow {
            is_signed: Some(v), ..
        }) => Metric::available(*v, Unit::None, Source::Wmi),
        Some(_) => Metric::unavailable(Unavailability::NotSupported),
        None => Metric::unavailable(Unavailability::Failed(AppError::new(
            ErrorCode::NotFound,
            "device not listed by Win32_PnPSignedDriver",
        ))),
    }
}

/// Builds one record. `row` is the WMI signature row for this device, if any; `wmi_error`
/// is set when the whole WMI query failed.
pub fn device_record(
    raw: RawDevice,
    row: Option<&SignedDriverRow>,
    wmi_error: Option<&AppError>,
) -> DeviceRecord {
    let name = raw
        .friendly_name
        .clone()
        .or_else(|| raw.description.clone())
        .unwrap_or_else(|| raw.instance_id.clone());
    let category = categorize(&CategoryInput {
        class_name: raw.class_name.as_deref(),
        class_guid: raw.class_guid.as_deref(),
        instance_id: &raw.instance_id,
        name: &name,
        hardware_ids: &raw.hardware_ids,
    });
    let (health, problem_code, problem_description) = problem::classify(&raw.status);

    let version = raw
        .driver_version
        .or_else(|| row.and_then(|r| r.driver_version.clone()));
    let date = raw
        .driver_date
        .or_else(|| row.and_then(|r| r.driver_date.clone()));
    let provider = raw
        .driver_provider
        .or_else(|| row.and_then(|r| r.provider.clone()));
    let inf_name = raw
        .driver_inf
        .or_else(|| row.and_then(|r| r.inf_name.clone()));
    let has_driver = version.is_some() || provider.is_some() || inf_name.is_some();
    let driver = has_driver.then(|| DriverInfo {
        is_inbox: is_inbox(provider.as_deref()),
        signed: signed_metric(row, wmi_error),
        signer: row.and_then(|r| r.signer.clone()),
        version,
        date,
        provider,
        inf_name,
    });

    DeviceRecord {
        instance_id: raw.instance_id,
        name,
        manufacturer: raw.manufacturer,
        class_name: raw.class_name,
        class_guid: raw.class_guid,
        category,
        hardware_ids: raw.hardware_ids,
        driver,
        health,
        problem_code,
        problem_description,
        update: UpdateStatus::NotChecked,
    }
}

fn health_rank(h: DeviceHealth) -> u8 {
    match h {
        DeviceHealth::DriverMissing => 0,
        DeviceHealth::ProblemDetected => 1,
        DeviceHealth::Disabled => 2,
        DeviceHealth::Working => 3,
    }
}

fn category_rank(c: DeviceCategory) -> u8 {
    c as u8
}

/// Problems first (missing driver, problem, disabled, working), then category, then name.
pub fn sort_devices(devices: &mut [DeviceRecord]) {
    devices.sort_by_cached_key(|d| {
        (
            health_rank(d.health),
            category_rank(d.category),
            d.name.to_lowercase(),
            d.instance_id.clone(),
        )
    });
}

/// Pure assembly of the inventory from the platform data.
pub fn build_inventory(
    raw: Vec<RawDevice>,
    signed: &Result<Vec<SignedDriverRow>, AppError>,
) -> DriverInventory {
    let (rows, wmi_error): (HashMap<String, &SignedDriverRow>, Option<&AppError>) = match signed {
        Ok(rows) => (
            rows.iter()
                .map(|r| (r.device_id.to_ascii_uppercase(), r))
                .collect(),
            None,
        ),
        Err(e) => (HashMap::new(), Some(e)),
    };
    let mut devices: Vec<DeviceRecord> = raw
        .into_iter()
        .map(|r| {
            let row = rows.get(&r.instance_id.to_ascii_uppercase()).copied();
            device_record(r, row, wmi_error)
        })
        .collect();
    sort_devices(&mut devices);
    DriverInventory {
        collected_at: now_unix_ms(),
        summary: DriverSummary::from_devices(&devices),
        devices,
        update_check: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform::devices::DevNodeStatus;

    fn raw(id: &str, class: &str, problem: Option<u32>, provider: Option<&str>) -> RawDevice {
        RawDevice {
            instance_id: id.into(),
            friendly_name: None,
            description: Some(format!("dev {id}")),
            manufacturer: None,
            class_name: Some(class.into()),
            class_guid: None,
            hardware_ids: vec![format!("HW\\{id}")],
            status: Ok(DevNodeStatus {
                started: problem.is_none(),
                problem_code: problem,
            }),
            driver_version: provider.map(|_| "1.0.0.0".into()),
            driver_date: provider.map(|_| "2006-06-21".into()),
            driver_provider: provider.map(Into::into),
            driver_inf: provider.map(|_| "x.inf".into()),
        }
    }

    fn row(id: &str, signed: Option<bool>) -> SignedDriverRow {
        SignedDriverRow {
            device_id: id.into(),
            is_signed: signed,
            signer: Some("Microsoft Windows".into()),
            driver_version: Some("9.9".into()),
            driver_date: Some("2020-01-01".into()),
            provider: Some("Microsoft".into()),
            inf_name: Some("machine.inf".into()),
        }
    }

    #[test]
    fn inbox_detection() {
        assert!(is_inbox(Some("Microsoft")));
        assert!(is_inbox(Some(" microsoft ")));
        assert!(!is_inbox(Some("Microsoft Corporation")));
        assert!(!is_inbox(Some("Intel")));
        assert!(!is_inbox(None));
    }

    #[test]
    fn generic_microsoft_driver_is_working_not_missing() {
        let r = device_record(
            raw("ACPI\\A", "System", None, Some("Microsoft")),
            None,
            None,
        );
        assert_eq!(r.health, DeviceHealth::Working);
        let d = r.driver.expect("driver");
        assert!(d.is_inbox);
        assert_eq!(r.category, DeviceCategory::Chipset);
    }

    #[test]
    fn signature_from_wmi_is_keyed_by_instance_id_case_insensitively() {
        let inv = build_inventory(
            vec![raw("PCI\\VEN_1\\a", "Display", None, Some("Intel"))],
            &Ok(vec![row("PCI\\VEN_1\\A", Some(true))]),
        );
        let d = inv.devices[0].driver.as_ref().expect("driver");
        assert_eq!(d.signed.value(), Some(&true));
        assert_eq!(d.signer.as_deref(), Some("Microsoft Windows"));
        // SetupAPI values win over WMI ones.
        assert_eq!(d.version.as_deref(), Some("1.0.0.0"));
        assert_eq!(d.provider.as_deref(), Some("Intel"));
    }

    #[test]
    fn wmi_failure_is_explained_and_other_data_kept() {
        let err = AppError::new(ErrorCode::Platform, "WMI timed out");
        let inv = build_inventory(
            vec![raw("PCI\\X", "Display", None, Some("Intel"))],
            &Err(err.clone()),
        );
        let d = inv.devices[0].driver.as_ref().expect("driver");
        assert_eq!(d.signed, Metric::unavailable(Unavailability::Failed(err)));
        assert_eq!(d.version.as_deref(), Some("1.0.0.0"));
        let denied = AppError::new(ErrorCode::PermissionDenied, "no");
        let inv = build_inventory(
            vec![raw("PCI\\X", "Display", None, Some("Intel"))],
            &Err(denied),
        );
        assert_eq!(
            inv.devices[0].driver.as_ref().expect("driver").signed,
            Metric::unavailable(Unavailability::PermissionDenied)
        );
    }

    #[test]
    fn device_without_driver_has_none() {
        let r = device_record(raw("PCI\\Y", "Other", Some(28), None), None, None);
        assert!(r.driver.is_none());
        assert_eq!(r.health, DeviceHealth::DriverMissing);
        assert_eq!(r.problem_code, Some(28));
        // A WMI row alone (no SetupAPI driver data) still yields driver info.
        let r = device_record(
            raw("PCI\\Z", "Other", None, None),
            Some(&row("PCI\\Z", None)),
            None,
        );
        let d = r.driver.expect("driver");
        assert_eq!(d.version.as_deref(), Some("9.9"));
        assert_eq!(d.signed, Metric::unavailable(Unavailability::NotSupported));
    }

    #[test]
    fn name_falls_back_to_description_then_instance_id() {
        let mut r = raw("PCI\\N", "System", None, None);
        r.friendly_name = Some("Friendly".into());
        assert_eq!(device_record(r.clone(), None, None).name, "Friendly");
        r.friendly_name = None;
        assert_eq!(device_record(r.clone(), None, None).name, "dev PCI\\N");
        r.description = None;
        assert_eq!(device_record(r, None, None).name, "PCI\\N");
    }

    #[test]
    fn sorted_problems_first_then_category_then_name() {
        let inv = build_inventory(
            vec![
                raw("B", "Display", None, Some("Intel")),
                raw("A", "Display", None, Some("Intel")),
                raw("D", "System", Some(22), Some("Microsoft")),
                raw("C", "USB", Some(43), Some("Microsoft")),
                raw("E", "Net", Some(28), None),
                raw("F", "System", None, Some("Microsoft")),
            ],
            &Ok(vec![]),
        );
        let order: Vec<&str> = inv.devices.iter().map(|d| d.instance_id.as_str()).collect();
        assert_eq!(order, ["E", "C", "D", "A", "B", "F"]);
    }

    #[test]
    fn summary_and_update_search_integration() {
        let inv = build_inventory(
            vec![
                raw("PCI\\GPU", "Display", None, Some("Intel")),
                raw("PCI\\NIC", "Net", Some(28), None),
                raw("ACPI\\SYS", "System", Some(22), Some("Microsoft")),
                raw("USB\\HUB", "USB", None, Some("Microsoft")),
            ],
            &Ok(vec![]),
        );
        let s = &inv.summary;
        assert_eq!(
            (
                s.total,
                s.working,
                s.problems,
                s.driver_missing,
                s.disabled,
                s.not_checked
            ),
            (4, 2, 0, 1, 1, 4)
        );
        assert!(inv.update_check.is_none());

        let offers = vec![DriverUpdateOffer {
            title: "Intel - Display - 31.0.101.5186".into(),
            hardware_id: "hw\\pci\\gpu".into(),
            provider: None,
            manufacturer: None,
            model: None,
            class: None,
            date: Some("2024-05-01".into()),
        }];
        let checked = apply_update_search(&inv, &offers, 123, 456, Language::Italian);
        let s = &checked.summary;
        assert_eq!(
            (
                s.updates_available,
                s.not_verifiable,
                s.not_checked,
                s.total
            ),
            (1, 3, 0, 4)
        );
        let uc = checked.update_check.as_ref().expect("update_check");
        assert_eq!((uc.checked_at, uc.duration_ms, uc.offered), (123, 456, 1));
        assert!(uc.error.is_none());
        // Health is untouched by the update check.
        assert_eq!(
            inv.devices.iter().map(|d| d.health).collect::<Vec<_>>(),
            checked.devices.iter().map(|d| d.health).collect::<Vec<_>>()
        );
    }

    /// Host smoke test: real SetupAPI + WMI on this machine.
    #[cfg(windows)]
    #[test]
    fn host_collect_inventory() {
        let t = Instant::now();
        let inv = collect_inventory().expect("collect_inventory");
        let elapsed = t.elapsed();
        assert!(inv.devices.len() > 20, "only {} devices", inv.devices.len());
        assert!(
            inv.devices
                .iter()
                .any(|d| d.health == DeviceHealth::Working)
        );
        assert!(inv.devices.iter().all(|d| !d.instance_id.trim().is_empty()));
        assert!(
            inv.devices
                .iter()
                .all(|d| d.update == UpdateStatus::NotChecked)
        );
        assert_eq!(inv.summary.total as usize, inv.devices.len());
        eprintln!(
            "collect_inventory: {} devices in {elapsed:?}; summary {:?}",
            inv.devices.len(),
            inv.summary
        );
    }

    /// Real Windows Update search (network, minutes). Run manually:
    /// `cargo test -p drivers -- --ignored --nocapture host_windows_update_search`
    #[cfg(windows)]
    #[test]
    #[ignore = "network: real Windows Update search, may take minutes"]
    fn host_windows_update_search() {
        let inv = collect_inventory().expect("collect_inventory");
        match check_updates(&inv, Language::English) {
            Ok(checked) => {
                let uc = checked.update_check.as_ref().expect("update_check");
                eprintln!(
                    "WU search: {} ms, offered {}, matched {}",
                    uc.duration_ms,
                    uc.offered,
                    updates::matched_count(&checked.devices)
                );
                for d in &checked.devices {
                    if let UpdateStatus::UpdateAvailable {
                        title,
                        version,
                        date,
                        hardware_id,
                        ..
                    } = &d.update
                    {
                        eprintln!(
                            "  {} <- {title} ({version:?}, {date:?}) via {hardware_id}",
                            d.name
                        );
                    }
                }
                assert!(checked.devices.iter().all(|d| !matches!(
                    d.update,
                    UpdateStatus::Verified { .. } | UpdateStatus::NotChecked
                )));
            }
            Err(e) => panic!("Windows Update search failed: {e:?}"),
        }
    }

    /// Outside Windows both entry points fail fast with the recognisable `NotSupported`
    /// error, without touching the platform layer.
    #[cfg(not(windows))]
    #[test]
    fn host_inventory_is_not_supported() {
        assert!(!inventory_supported());
        let e = collect_inventory().expect_err("not supported");
        assert_eq!(e.code, ErrorCode::NotSupported);
        assert_eq!(e.message, UNSUPPORTED_MESSAGE);
        assert!(!e.retriable);
        let empty = build_inventory(Vec::new(), &Ok(Vec::new()));
        let e = check_updates(&empty, Language::Italian).expect_err("not supported");
        assert_eq!(e.code, ErrorCode::NotSupported);
    }
}
