//! Windows Update Agent driver search (`IUpdateSearcher::Search`). SEARCH ONLY.
//!
//! This module never creates an `IUpdateDownloader` or `IUpdateInstaller`: nothing is
//! downloaded, installed or executed. The search itself contacts the configured update
//! service (Windows Update, or WSUS when policy says so) and can take minutes.
//!
//! COM is initialized as MTA on a dedicated thread created here, so the caller's
//! apartment is never touched. The synchronous `Search` cannot be interrupted: on timeout
//! the caller gets an error and the worker finishes (and uninitializes COM) on its own.

use std::sync::mpsc;
use std::time::Duration;

use ::windows::Win32::Foundation::{E_ACCESSDENIED, VARIANT_TRUE};
use ::windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use ::windows::Win32::System::UpdateAgent::{
    IUpdateSession, IWindowsDriverUpdate, UpdateSession, orcAborted, orcFailed,
};
use ::windows::core::{BSTR, Interface};
use models::{AppError, ErrorCode};

use crate::devices::{DriverUpdateOffer, ole_date_to_iso_date};

/// Not-installed driver updates only. `Type='Driver'` excludes software updates.
const CRITERIA: &str = "IsInstalled=0 and Type='Driver'";
const CLIENT_ID: &str = "PC Diagnostics (read-only driver check)";

/// Balances a successful `CoInitializeEx` on this thread.
struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: only constructed after CoInitializeEx succeeded on this same thread.
        unsafe { CoUninitialize() };
    }
}

fn hr_error(op: &str, e: &::windows::core::Error) -> AppError {
    let detail = format!("HRESULT 0x{:08X}: {}", e.code().0, e.message());
    if e.code() == E_ACCESSDENIED {
        return AppError::new(ErrorCode::PermissionDenied, format!("{op}: access denied"))
            .with_detail(detail);
    }
    // Network and service errors (WU_E_*) are transient from the user's point of view.
    AppError::new(ErrorCode::Platform, format!("{op} failed")).with_detail(detail)
}

fn bstr(r: ::windows::core::Result<BSTR>) -> Option<String> {
    let s = r.ok()?.to_string();
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

fn search_blocking() -> Result<Vec<DriverUpdateOffer>, AppError> {
    // SAFETY: initializes COM on this dedicated worker thread only; balanced by ComGuard.
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if hr.is_err() {
        return Err(
            AppError::new(ErrorCode::Platform, "CoInitializeEx(MTA) failed")
                .with_detail(format!("HRESULT 0x{:08X}", hr.0)),
        );
    }
    // Declared before every COM object so it is dropped (CoUninitialize) after them.
    let _com = ComGuard;

    // SAFETY: static CLSID; COM is initialized on this thread.
    let session: IUpdateSession =
        unsafe { CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER) }
            .map_err(|e| hr_error("CoCreateInstance(UpdateSession)", &e))?;
    // SAFETY: valid session; the BSTR outlives the call. Cosmetic: failure is harmless.
    if let Err(e) = unsafe { session.SetClientApplicationID(&BSTR::from(CLIENT_ID)) } {
        tracing::debug!(error = %e, "IUpdateSession::SetClientApplicationID failed");
    }
    // SAFETY: valid session.
    let searcher = unsafe { session.CreateUpdateSearcher() }
        .map_err(|e| hr_error("IUpdateSession::CreateUpdateSearcher", &e))?;
    // SAFETY: valid searcher. Online search (the default) so the result reflects what the
    // update service offers now, not a stale local cache.
    if let Err(e) = unsafe { searcher.SetOnline(VARIANT_TRUE) } {
        tracing::debug!(error = %e, "IUpdateSearcher::SetOnline failed");
    }
    // SAFETY: valid searcher; the criteria BSTR outlives the call.
    let result = unsafe { searcher.Search(&BSTR::from(CRITERIA)) }
        .map_err(|e| hr_error("IUpdateSearcher::Search", &e))?;
    // SAFETY: valid search result.
    let code =
        unsafe { result.ResultCode() }.map_err(|e| hr_error("ISearchResult::ResultCode", &e))?;
    if code == orcFailed || code == orcAborted {
        return Err(AppError::new(
            ErrorCode::Platform,
            "Windows Update search did not complete",
        )
        .with_detail(format!("OperationResultCode {}", code.0)));
    }
    // SAFETY: valid search result.
    let updates =
        unsafe { result.Updates() }.map_err(|e| hr_error("ISearchResult::Updates", &e))?;
    // SAFETY: valid collection.
    let count = unsafe { updates.Count() }.map_err(|e| hr_error("IUpdateCollection::Count", &e))?;

    let mut out = Vec::new();
    for i in 0..count {
        // SAFETY: valid collection; 0 <= i < Count.
        let update = match unsafe { updates.get_Item(i) } {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(index = i, error = %e, "IUpdateCollection::Item failed");
                continue;
            }
        };
        let Ok(driver) = update.cast::<IWindowsDriverUpdate>() else {
            tracing::debug!(index = i, "update is not an IWindowsDriverUpdate; skipped");
            continue;
        };
        // SAFETY (all getters below): valid IWindowsDriverUpdate; each returns an owned
        // BSTR / value.
        let Some(hardware_id) = bstr(unsafe { driver.DriverHardwareID() }) else {
            tracing::debug!(index = i, "driver update without hardware id; skipped");
            continue;
        };
        out.push(DriverUpdateOffer {
            title: bstr(unsafe { driver.Title() }).unwrap_or_default(),
            hardware_id,
            provider: bstr(unsafe { driver.DriverProvider() }),
            manufacturer: bstr(unsafe { driver.DriverManufacturer() }),
            model: bstr(unsafe { driver.DriverModel() }),
            class: bstr(unsafe { driver.DriverClass() }),
            date: unsafe { driver.DriverVerDate() }
                .ok()
                .and_then(ole_date_to_iso_date),
        });
    }
    Ok(out)
}

/// Runs the search on a dedicated MTA thread; gives up after `timeout` (retriable error).
pub fn search_driver_updates(timeout: Duration) -> Result<Vec<DriverUpdateOffer>, AppError> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("wua-driver-search".into())
        .spawn(move || {
            let _ = tx.send(search_blocking());
        })
        .map_err(|e| {
            AppError::internal("could not spawn Windows Update worker thread")
                .with_detail(e.to_string())
        })?;
    match rx.recv_timeout(timeout) {
        Ok(r) => r,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let mut e = AppError::new(ErrorCode::Platform, "Windows Update search timed out")
                .with_detail(format!("no answer within {} s", timeout.as_secs()));
            e.retriable = true;
            Err(e)
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(AppError::internal(
            "Windows Update worker thread ended without a result",
        )),
    }
}
