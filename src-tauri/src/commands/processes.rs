use models::AppError;
use models::process::{ProcessHistory, ProcessSnapshot};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_process_snapshot(state: State<'_, AppState>) -> Result<ProcessSnapshot, AppError> {
    state.processes()?.snapshot()
}

#[tauri::command]
pub async fn get_process_history(
    state: State<'_, AppState>,
    key: String,
) -> Result<ProcessHistory, AppError> {
    // Keys are "<pid>:<creation ms>"; reject anything oversized before touching the monitor.
    if key.len() > 64 {
        return Err(AppError::invalid_input("process key too long"));
    }
    state.processes()?.history(&key)
}
