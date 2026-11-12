//! WMI inventory queries: RAM modules, physical disks and the system enclosure (chassis
//! type, for the form factor).
//!
//! WMI is COM. All queries run on a dedicated short-lived thread, so the caller's COM
//! apartment (e.g. a Tauri/WebView STA thread) is never touched. The `wmi` crate joins
//! the multithreaded apartment on that thread.

use std::sync::mpsc;
use std::time::Duration;

use models::hardware::{MemoryModule, PhysicalDiskInfo};
use models::metric::{Metric, Source, Unavailability, Unit};
use models::{AppError, ErrorCode};
use serde::Deserialize;
use wmi::{WMIConnection, WMIError};

use crate::convert::{
    bus_kind_from_msft, health_from_msft, media_kind_from_msft, memory_form_factor, non_empty,
};

/// WMI can stall (e.g. a broken provider). After this the result is reported as failed and
/// the worker thread is left to finish on its own.
const WMI_TIMEOUT: Duration = Duration::from_secs(20);

const WBEM_E_ACCESS_DENIED: i32 = 0x8004_1003_u32 as i32;
const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32PhysicalMemory {
    capacity: Option<u64>,
    speed: Option<u32>,
    configured_clock_speed: Option<u32>,
    manufacturer: Option<String>,
    part_number: Option<String>,
    form_factor: Option<u16>,
    device_locator: Option<String>,
    bank_label: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MsftPhysicalDisk {
    device_id: Option<String>,
    friendly_name: Option<String>,
    firmware_version: Option<String>,
    media_type: Option<u16>,
    bus_type: Option<u16>,
    size: Option<u64>,
    health_status: Option<u16>,
}

fn wmi_reason(op: &str, e: &WMIError) -> Unavailability {
    if let WMIError::HResultError { hres } = e
        && (*hres == WBEM_E_ACCESS_DENIED || *hres == E_ACCESSDENIED)
    {
        return Unavailability::PermissionDenied;
    }
    Unavailability::Failed(
        AppError::new(ErrorCode::Platform, format!("WMI {op} failed")).with_detail(e.to_string()),
    )
}

fn memory_modules_blocking() -> Metric<Vec<MemoryModule>> {
    let con = match WMIConnection::new() {
        Ok(c) => c,
        Err(e) => return Metric::unavailable(wmi_reason("connect ROOT\\CIMV2", &e)),
    };
    let rows: Vec<Win32PhysicalMemory> = match con.raw_query(
        "SELECT Capacity, Speed, ConfiguredClockSpeed, Manufacturer, PartNumber, FormFactor, \
         DeviceLocator, BankLabel FROM Win32_PhysicalMemory",
    ) {
        Ok(r) => r,
        Err(e) => return Metric::unavailable(wmi_reason("Win32_PhysicalMemory", &e)),
    };
    let modules = rows
        .into_iter()
        .map(|r| {
            let slot = match (non_empty(r.bank_label), non_empty(r.device_locator)) {
                (Some(b), Some(d)) if b != d => Some(format!("{b} / {d}")),
                (_, Some(d)) => Some(d),
                (b, None) => b,
            };
            MemoryModule {
                capacity_bytes: r.capacity.unwrap_or(0),
                speed_mts: r.speed.filter(|s| *s > 0),
                configured_speed_mts: r.configured_clock_speed.filter(|s| *s > 0),
                manufacturer: non_empty(r.manufacturer),
                part_number: non_empty(r.part_number),
                form_factor: r
                    .form_factor
                    .and_then(memory_form_factor)
                    .map(str::to_owned),
                slot,
            }
        })
        .collect();
    Metric::available(modules, Unit::None, Source::Wmi)
}

pub(crate) fn physical_disks_blocking() -> Metric<Vec<PhysicalDiskInfo>> {
    let con = match WMIConnection::with_namespace_path("ROOT\\Microsoft\\Windows\\Storage") {
        Ok(c) => c,
        Err(e) => {
            return Metric::unavailable(wmi_reason(
                "connect ROOT\\Microsoft\\Windows\\Storage",
                &e,
            ));
        }
    };
    let rows: Vec<MsftPhysicalDisk> = match con.raw_query(
        "SELECT DeviceId, FriendlyName, FirmwareVersion, MediaType, BusType, Size, HealthStatus \
         FROM MSFT_PhysicalDisk",
    ) {
        Ok(r) => r,
        Err(e) => return Metric::unavailable(wmi_reason("MSFT_PhysicalDisk", &e)),
    };
    let disks = rows
        .into_iter()
        .map(|r| PhysicalDiskInfo {
            id: r.device_id.unwrap_or_default(),
            model: non_empty(r.friendly_name),
            firmware: non_empty(r.firmware_version),
            media: media_kind_from_msft(r.media_type.unwrap_or(0)),
            bus: bus_kind_from_msft(r.bus_type.unwrap_or(0)),
            size_bytes: r.size.unwrap_or(0),
            health: match r.health_status {
                Some(h) => Metric::available(health_from_msft(h), Unit::None, Source::Wmi),
                None => Metric::unavailable(Unavailability::NotSupported),
            },
        })
        .collect();
    Metric::available(disks, Unit::None, Source::Wmi)
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32SystemEnclosure {
    chassis_types: Option<Vec<u16>>,
}

/// SMBIOS chassis type codes of every `Win32_SystemEnclosure` (normally one), readable
/// without admin. Mapped by `crate::form_factor::from_chassis_types`.
fn chassis_types_blocking() -> Result<Vec<u16>, Unavailability> {
    let con = WMIConnection::new().map_err(|e| wmi_reason("connect ROOT\\CIMV2", &e))?;
    let rows: Vec<Win32SystemEnclosure> = con
        .raw_query("SELECT ChassisTypes FROM Win32_SystemEnclosure")
        .map_err(|e| wmi_reason("Win32_SystemEnclosure", &e))?;
    Ok(rows
        .into_iter()
        .flat_map(|r| r.chassis_types.unwrap_or_default())
        .collect())
}

pub struct WmiInventory {
    pub modules: Metric<Vec<MemoryModule>>,
    pub physical_disks: Metric<Vec<PhysicalDiskInfo>>,
    pub chassis_types: Result<Vec<u16>, Unavailability>,
}

/// Runs `f` on a dedicated thread and waits (bounded) for its result; `failed` builds the
/// result when the thread cannot start or times out.
fn on_wmi_thread<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
    failed: impl FnOnce(Unavailability) -> T,
) -> T {
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("wmi-inventory".into())
        .spawn(move || {
            let _ = tx.send(f());
        });
    let reason =
        |message: &str| Unavailability::Failed(AppError::new(ErrorCode::Platform, message));
    if let Err(e) = spawned {
        tracing::error!(error = %e, "could not spawn WMI thread");
        return failed(reason("could not spawn WMI worker thread"));
    }
    match rx.recv_timeout(WMI_TIMEOUT) {
        Ok(v) => v,
        Err(_) => {
            tracing::error!("WMI inventory timed out");
            failed(reason("WMI inventory query timed out"))
        }
    }
}

/// Runs the inventory WMI queries on a dedicated thread and waits (bounded) for the result.
pub fn collect() -> WmiInventory {
    on_wmi_thread(
        || WmiInventory {
            modules: memory_modules_blocking(),
            physical_disks: physical_disks_blocking(),
            chassis_types: chassis_types_blocking(),
        },
        |r| WmiInventory {
            modules: Metric::unavailable(r.clone()),
            physical_disks: Metric::unavailable(r.clone()),
            chassis_types: Err(r),
        },
    )
}

/// The chassis query alone (for `crate::form_factor::detect`).
pub fn chassis_types() -> Result<Vec<u16>, Unavailability> {
    on_wmi_thread(chassis_types_blocking, Err)
}
