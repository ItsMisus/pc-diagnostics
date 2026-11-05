use models::AppError;
use models::hardware::HardwareInventory;
use models::monitor::{AppOverhead, LiveSample, MonitorStatus};
use tauri::State;

use super::blocking;
use crate::state::AppState;

/// Cached inventory; `refresh` re-runs the slow (WMI/DXGI) collection off the IPC thread.
#[tauri::command]
pub async fn get_hardware_inventory(
    state: State<'_, AppState>,
    refresh: bool,
) -> Result<HardwareInventory, AppError> {
    let monitor = state.monitor()?;
    blocking(move || monitor.inventory(refresh)).await
}

#[tauri::command]
pub async fn get_live_history(state: State<'_, AppState>) -> Result<Vec<LiveSample>, AppError> {
    Ok(state.monitor()?.history())
}

#[tauri::command]
pub async fn get_monitor_status(state: State<'_, AppState>) -> Result<MonitorStatus, AppError> {
    Ok(state.monitor()?.status())
}

#[tauri::command]
pub async fn get_app_overhead(state: State<'_, AppState>) -> Result<AppOverhead, AppError> {
    Ok(state.monitor()?.app_overhead())
}
