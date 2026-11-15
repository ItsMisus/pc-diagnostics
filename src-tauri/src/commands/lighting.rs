//! IPC commands of the lighting center. Owned by the lighting feature.
//!
//! Thin: read the language, run the controller on a blocking thread. Validation of effects,
//! brightness and names happens in `control::lighting`.

use std::sync::Arc;

use control::lighting::LightingController;
use models::AppError;
use models::control::lighting::{LightingSetup, LightingState};
use tauri::State;

use super::blocking;
use crate::state::AppState;

type Lighting<'a> = State<'a, Arc<LightingController>>;

#[tauri::command]
pub async fn lighting_get_state(
    app: State<'_, AppState>,
    lighting: Lighting<'_>,
) -> Result<LightingState, AppError> {
    let (lang, c) = (app.language(), lighting.inner().clone());
    blocking(move || c.state(lang)).await
}

#[tauri::command]
pub async fn lighting_refresh(
    app: State<'_, AppState>,
    lighting: Lighting<'_>,
) -> Result<LightingState, AppError> {
    let (lang, c) = (app.language(), lighting.inner().clone());
    blocking(move || c.refresh(lang)).await
}

#[tauri::command]
pub async fn lighting_apply_profile(
    app: State<'_, AppState>,
    lighting: Lighting<'_>,
    id: String,
) -> Result<LightingState, AppError> {
    let (lang, c) = (app.language(), lighting.inner().clone());
    blocking(move || c.apply_profile(&id, lang)).await
}

#[tauri::command]
pub async fn lighting_apply_setup(
    app: State<'_, AppState>,
    lighting: Lighting<'_>,
    setup: LightingSetup,
) -> Result<LightingState, AppError> {
    let (lang, c) = (app.language(), lighting.inner().clone());
    blocking(move || c.apply_setup(setup, lang)).await
}

#[tauri::command]
pub async fn lighting_preview_setup(
    app: State<'_, AppState>,
    lighting: Lighting<'_>,
    setup: LightingSetup,
) -> Result<LightingState, AppError> {
    let (lang, c) = (app.language(), lighting.inner().clone());
    blocking(move || c.preview_setup(setup, lang)).await
}

#[tauri::command]
pub async fn lighting_set_brightness(
    app: State<'_, AppState>,
    lighting: Lighting<'_>,
    brightness: u8,
) -> Result<LightingState, AppError> {
    let (lang, c) = (app.language(), lighting.inner().clone());
    blocking(move || c.set_brightness(brightness, lang)).await
}

#[tauri::command]
pub async fn lighting_save_profile(
    app: State<'_, AppState>,
    lighting: Lighting<'_>,
    id: Option<String>,
    name: String,
    setup: LightingSetup,
) -> Result<LightingState, AppError> {
    let (lang, c) = (app.language(), lighting.inner().clone());
    blocking(move || c.save_profile(id, &name, setup, lang)).await
}

#[tauri::command]
pub async fn lighting_delete_profile(
    app: State<'_, AppState>,
    lighting: Lighting<'_>,
    id: String,
) -> Result<LightingState, AppError> {
    let (lang, c) = (app.language(), lighting.inner().clone());
    blocking(move || c.delete_profile(&id, lang)).await
}

#[tauri::command]
pub async fn lighting_restore(
    app: State<'_, AppState>,
    lighting: Lighting<'_>,
) -> Result<LightingState, AppError> {
    let (lang, c) = (app.language(), lighting.inner().clone());
    blocking(move || c.restore(lang)).await
}
