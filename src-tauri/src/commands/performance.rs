//! IPC commands of the performance center. Owned by the performance feature.
//! Thin: the controller validates, persists and talks to the OS (on `blocking`).

use std::sync::Arc;

use control::performance::PerformanceController;
use models::AppError;
use models::control::performance::{PerformanceProfileInput, PerformanceState};
use tauri::State;

use super::blocking;
use crate::state::AppState;

type Controller<'a> = State<'a, Arc<PerformanceController>>;

#[tauri::command]
pub async fn get_performance_state(
    state: State<'_, AppState>,
    performance: Controller<'_>,
) -> Result<PerformanceState, AppError> {
    let c = Arc::clone(&performance);
    let lang = state.language();
    blocking(move || c.state(lang)).await
}

/// Applies a profile (built-in or user) for the current power supply; it stays selected.
#[tauri::command]
pub async fn apply_performance_profile(
    state: State<'_, AppState>,
    performance: Controller<'_>,
    profile_id: String,
) -> Result<PerformanceState, AppError> {
    let c = Arc::clone(&performance);
    let lang = state.language();
    blocking(move || c.apply_profile(&profile_id, lang)).await
}

/// Creates (`id` null) or replaces a user profile.
#[tauri::command]
pub async fn save_performance_profile(
    state: State<'_, AppState>,
    performance: Controller<'_>,
    profile: PerformanceProfileInput,
) -> Result<PerformanceState, AppError> {
    let c = Arc::clone(&performance);
    let lang = state.language();
    blocking(move || c.save_profile(profile, lang)).await
}

#[tauri::command]
pub async fn delete_performance_profile(
    state: State<'_, AppState>,
    performance: Controller<'_>,
    profile_id: String,
) -> Result<PerformanceState, AppError> {
    let c = Arc::clone(&performance);
    let lang = state.language();
    blocking(move || c.delete_profile(&profile_id, lang)).await
}

/// Puts the system state from before the first change back and clears the selection.
#[tauri::command]
pub async fn restore_performance_original(
    state: State<'_, AppState>,
    performance: Controller<'_>,
) -> Result<PerformanceState, AppError> {
    let c = Arc::clone(&performance);
    let lang = state.language();
    blocking(move || c.restore_original(lang)).await
}

/// Opens the OS power settings (fixed target, no argument from the frontend).
#[tauri::command]
pub async fn open_power_settings(performance: Controller<'_>) -> Result<(), AppError> {
    let c = Arc::clone(&performance);
    blocking(move || c.open_power_settings()).await
}
