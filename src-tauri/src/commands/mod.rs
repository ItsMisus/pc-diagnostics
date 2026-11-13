//! IPC commands. Thin layer: validate input, delegate to crates, map errors to `AppError`.
//! Blocking work always runs on `spawn_blocking`, never on the IPC/main thread.

pub mod app;
pub mod assistant;
pub mod bench;
pub mod cooling;
pub mod diagnostics;
pub mod drivers;
pub mod gaming;
pub mod lighting;
pub mod monitor;
pub mod performance;
pub mod processes;
pub mod settings;
pub mod system;

use models::AppError;

/// Runs blocking work off the async runtime and flattens join errors.
pub(crate) async fn blocking<T, F>(f: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::internal("background task failed").with_detail(e.to_string()))?
}
