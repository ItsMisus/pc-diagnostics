use models::drivers::DriverInventory;
use models::{AppError, ErrorCode};
use tauri::State;

use super::blocking;
use crate::state::AppState;

/// Cached device inventory; `refresh` re-enumerates devices (local only, no network).
/// Update-status reasons follow the current language even when cached. Windows only:
/// elsewhere (macOS) it rejects with `NotSupported` (`drivers::unsupported_error`).
#[tauri::command]
pub async fn get_driver_inventory(
    state: State<'_, AppState>,
    refresh: bool,
) -> Result<DriverInventory, AppError> {
    if !drivers::inventory_supported() {
        return Err(drivers::unsupported_error());
    }
    if !refresh && let Some(mut cached) = state.driver_inventory()? {
        drivers::updates::localize_reasons(&mut cached.devices, state.language());
        return Ok(cached);
    }
    let inventory = blocking(drivers::collect_inventory).await?;
    state.set_driver_inventory(inventory.clone())?;
    Ok(inventory)
}

/// Searches Windows Update for driver updates. Requires the "online checks" opt-in;
/// only one search runs at a time. Read-only: nothing is installed. Windows only:
/// elsewhere it rejects with `NotSupported` before looking at the opt-in.
#[tauri::command]
pub async fn check_driver_updates(state: State<'_, AppState>) -> Result<DriverInventory, AppError> {
    if !drivers::inventory_supported() {
        return Err(drivers::unsupported_error());
    }
    if !state.db()?.load_settings()?.allow_online_checks {
        return Err(AppError::new(
            ErrorCode::ConsentRequired,
            "online checks are disabled in settings",
        ));
    }
    let _guard = state.begin_update_check()?;

    let base = match state.driver_inventory()? {
        Some(inv) => inv,
        None => blocking(drivers::collect_inventory).await?,
    };
    let lang = state.language();
    let checked = blocking(move || drivers::check_updates(&base, lang)).await?;
    state.set_driver_inventory(checked.clone())?;
    Ok(checked)
}
