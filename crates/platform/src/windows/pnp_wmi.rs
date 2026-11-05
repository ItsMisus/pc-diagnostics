//! Driver signature data from WMI `Win32_PnPSignedDriver`.
//!
//! The class is slow (it walks every driver-bound device: seconds). It runs once per
//! inventory on a dedicated short-lived thread so the caller's COM apartment is never
//! touched; the `wmi` crate joins the MTA on that thread.

use std::sync::mpsc;
use std::time::Duration;

use models::{AppError, ErrorCode};
use serde::Deserialize;
use wmi::{WMIConnection, WMIError};

use crate::convert::non_empty;
use crate::devices::{SignedDriverRow, cim_datetime_to_iso_date};

const WBEM_E_ACCESS_DENIED: i32 = 0x8004_1003_u32 as i32;
const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Win32PnPSignedDriver {
    #[serde(rename = "DeviceID")]
    device_id: Option<String>,
    is_signed: Option<bool>,
    signer: Option<String>,
    driver_version: Option<String>,
    driver_date: Option<String>,
    driver_provider_name: Option<String>,
    inf_name: Option<String>,
}

fn wmi_error(op: &str, e: &WMIError) -> AppError {
    if let WMIError::HResultError { hres } = e
        && (*hres == WBEM_E_ACCESS_DENIED || *hres == E_ACCESSDENIED)
    {
        return AppError::new(
            ErrorCode::PermissionDenied,
            format!("WMI {op}: access denied"),
        )
        .with_detail(e.to_string());
    }
    AppError::new(ErrorCode::Platform, format!("WMI {op} failed")).with_detail(e.to_string())
}

fn query_blocking() -> Result<Vec<SignedDriverRow>, AppError> {
    let con = WMIConnection::new().map_err(|e| wmi_error("connect ROOT\\CIMV2", &e))?;
    let rows: Vec<Win32PnPSignedDriver> = con
        .raw_query(
            "SELECT DeviceID, IsSigned, Signer, DriverVersion, DriverDate, DriverProviderName, \
             InfName FROM Win32_PnPSignedDriver",
        )
        .map_err(|e| wmi_error("Win32_PnPSignedDriver", &e))?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(SignedDriverRow {
                device_id: non_empty(r.device_id)?,
                is_signed: r.is_signed,
                signer: non_empty(r.signer),
                driver_version: non_empty(r.driver_version),
                driver_date: r.driver_date.as_deref().and_then(cim_datetime_to_iso_date),
                provider: non_empty(r.driver_provider_name),
                inf_name: non_empty(r.inf_name),
            })
        })
        .collect())
}

/// Runs the query on its own thread; gives up after `timeout`.
pub fn signed_drivers(timeout: Duration) -> Result<Vec<SignedDriverRow>, AppError> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("wmi-pnp-signed".into())
        .spawn(move || {
            let _ = tx.send(query_blocking());
        })
        .map_err(|e| {
            AppError::internal("could not spawn WMI worker thread").with_detail(e.to_string())
        })?;
    match rx.recv_timeout(timeout) {
        Ok(r) => r,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(AppError::new(
            ErrorCode::Platform,
            "WMI Win32_PnPSignedDriver query timed out",
        )
        .with_detail(format!("no answer within {} s", timeout.as_secs()))),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(AppError::internal(
            "WMI worker thread ended without a result",
        )),
    }
}
