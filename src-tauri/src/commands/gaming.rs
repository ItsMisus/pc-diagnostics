//! IPC commands of the gaming center. Owned by the gaming feature.
//!
//! Thin: the frontend names games by library id only (never a path to launch); the one path
//! it sends, `add_manual_game`, comes from the native file picker and is validated by the
//! platform layer (existing `.app` bundle on macOS, `.exe` on Windows).

use std::sync::Arc;

use control::gaming::GamingController;
use models::AppError;
use models::control::gaming::{GameArtKind, GameProfile, GamingState};
use tauri::State;

use super::blocking;
use crate::state::AppState;

type Gaming<'a> = State<'a, Arc<GamingController>>;

const MAX_ID_LEN: usize = 512;

fn check_id(id: &str) -> Result<(), AppError> {
    if id.is_empty() || id.len() > MAX_ID_LEN || id.chars().any(char::is_control) {
        return Err(AppError::invalid_input("invalid game id"));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_gaming_state(
    state: State<'_, AppState>,
    gaming: Gaming<'_>,
    refresh: Option<bool>,
) -> Result<GamingState, AppError> {
    let lang = state.language();
    let ctl = Arc::clone(&gaming);
    blocking(move || ctl.state(lang, refresh.unwrap_or(false))).await
}

#[tauri::command]
pub async fn get_game_art(
    gaming: Gaming<'_>,
    id: String,
    kind: GameArtKind,
) -> Result<Option<String>, AppError> {
    check_id(&id)?;
    let ctl = Arc::clone(&gaming);
    blocking(move || ctl.art(&id, kind)).await
}

#[tauri::command]
pub async fn launch_game(
    state: State<'_, AppState>,
    gaming: Gaming<'_>,
    id: String,
    with_profile: bool,
) -> Result<GamingState, AppError> {
    check_id(&id)?;
    let lang = state.language();
    let ctl = Arc::clone(&gaming);
    blocking(move || ctl.launch(&id, with_profile, lang)).await
}

#[tauri::command]
pub async fn add_manual_game(
    state: State<'_, AppState>,
    gaming: Gaming<'_>,
    path: String,
) -> Result<GamingState, AppError> {
    if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
        return Err(AppError::invalid_input("invalid path"));
    }
    let lang = state.language();
    let ctl = Arc::clone(&gaming);
    blocking(move || ctl.add_manual(&path, lang)).await
}

#[tauri::command]
pub async fn remove_manual_game(
    state: State<'_, AppState>,
    gaming: Gaming<'_>,
    id: String,
) -> Result<GamingState, AppError> {
    check_id(&id)?;
    let lang = state.language();
    let ctl = Arc::clone(&gaming);
    blocking(move || ctl.remove_manual(&id, lang)).await
}

#[tauri::command]
pub async fn set_game_hidden(
    state: State<'_, AppState>,
    gaming: Gaming<'_>,
    id: String,
    hidden: bool,
) -> Result<GamingState, AppError> {
    check_id(&id)?;
    let lang = state.language();
    let ctl = Arc::clone(&gaming);
    blocking(move || ctl.set_hidden(&id, hidden, lang)).await
}

#[tauri::command]
pub async fn set_game_profile(
    state: State<'_, AppState>,
    gaming: Gaming<'_>,
    profile: GameProfile,
) -> Result<GamingState, AppError> {
    check_id(&profile.game_id)?;
    let lang = state.language();
    let ctl = Arc::clone(&gaming);
    blocking(move || ctl.set_profile(profile, lang)).await
}

#[tauri::command]
pub async fn set_game_mode(
    state: State<'_, AppState>,
    gaming: Gaming<'_>,
    enabled: bool,
) -> Result<GamingState, AppError> {
    let lang = state.language();
    let ctl = Arc::clone(&gaming);
    blocking(move || ctl.set_game_mode(enabled, lang)).await
}

#[tauri::command]
pub async fn stop_game_session(
    state: State<'_, AppState>,
    gaming: Gaming<'_>,
) -> Result<GamingState, AppError> {
    let lang = state.language();
    let ctl = Arc::clone(&gaming);
    blocking(move || ctl.stop_session(lang)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_bounded() {
        assert!(check_id("steam:322170").is_ok());
        assert!(check_id("").is_err());
        assert!(check_id("a\nb").is_err());
        assert!(check_id(&"x".repeat(MAX_ID_LEN + 1)).is_err());
    }
}
