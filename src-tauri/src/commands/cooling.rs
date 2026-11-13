//! IPC commands of the cooling center. Owned by the cooling feature.
//! Thin: language from `AppState`, the work on `blocking`, the controller does the rest.

use std::sync::Arc;

use control::cooling::CoolingController;
use models::AppError;
use models::control::cooling::{CoolingProfileDraft, CoolingState};
use tauri::State;

use super::blocking;
use crate::state::AppState;

type Cooling<'a> = State<'a, Arc<CoolingController>>;

#[tauri::command]
pub async fn get_cooling_state(
    state: State<'_, AppState>,
    cooling: Cooling<'_>,
) -> Result<CoolingState, AppError> {
    let lang = state.language();
    let c = Arc::clone(&cooling);
    blocking(move || c.state(lang)).await
}

#[tauri::command]
pub async fn apply_cooling_profile(
    state: State<'_, AppState>,
    cooling: Cooling<'_>,
    id: String,
) -> Result<CoolingState, AppError> {
    let lang = state.language();
    let c = Arc::clone(&cooling);
    blocking(move || c.apply_profile(&id, lang)).await
}

#[tauri::command]
pub async fn save_cooling_profile(
    state: State<'_, AppState>,
    cooling: Cooling<'_>,
    draft: CoolingProfileDraft,
) -> Result<CoolingState, AppError> {
    let lang = state.language();
    let c = Arc::clone(&cooling);
    blocking(move || c.save_profile(draft, lang)).await
}

#[tauri::command]
pub async fn delete_cooling_profile(
    state: State<'_, AppState>,
    cooling: Cooling<'_>,
    id: String,
) -> Result<CoolingState, AppError> {
    let lang = state.language();
    let c = Arc::clone(&cooling);
    blocking(move || c.delete_profile(&id, lang)).await
}

#[tauri::command]
pub async fn restore_cooling(
    state: State<'_, AppState>,
    cooling: Cooling<'_>,
) -> Result<CoolingState, AppError> {
    let lang = state.language();
    let c = Arc::clone(&cooling);
    blocking(move || c.restore(lang)).await
}
