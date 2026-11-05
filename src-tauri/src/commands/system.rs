use models::AppError;
use models::system::SystemInfo;

use super::blocking;

#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, AppError> {
    blocking(|| Ok(platform::system::system_info())).await
}
