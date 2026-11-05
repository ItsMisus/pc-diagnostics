use models::AppError;
use models::settings::{AppSettings, Language};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    Ok(state.db()?.load_settings()?)
}

/// Validates and persists the full settings object, applies runtime-relevant values
/// (sampling interval) and returns what was stored.
#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, AppError> {
    settings.validate()?;
    let stored = {
        let db = state.db()?;
        db.save_settings(&settings)?;
        db.load_settings()?
    };
    state.monitor()?.set_interval(stored.sampling_interval_ms)?;
    state.processes()?.set_top_n(stored.process_top_n)?;
    Ok(stored)
}

/// Effective interface language: the stored preference, with `System` resolved against the
/// Windows display language. The UI uses it so frontend and backend texts always agree.
#[tauri::command]
pub async fn get_language(state: State<'_, AppState>) -> Result<Language, AppError> {
    Ok(state.language())
}
