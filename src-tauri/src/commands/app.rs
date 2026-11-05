use models::AppError;
use models::app::AppInfo;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_app_info(state: State<'_, AppState>) -> Result<AppInfo, AppError> {
    let schema_version = state.db()?.schema_version().map_err(AppError::from)?;
    Ok(AppInfo {
        name: env!("CARGO_PKG_NAME").to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        build_profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
        .to_owned(),
        schema_version,
    })
}
