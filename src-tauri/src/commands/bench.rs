use models::AppError;
use models::ErrorCode;
use models::benchmark::BenchmarkKind;
use models::benchmark_run::{
    BENCHMARK_FINISHED_EVENT, BENCHMARK_PROGRESS_EVENT, BenchmarkCapability, BenchmarkProgress,
    BenchmarkRequest, BenchmarkResult,
};
use models::diagnostics::BenchmarkComparison;
use tauri::{AppHandle, Emitter, Manager, State};

use super::blocking;
use crate::state::AppState;

#[tauri::command]
pub async fn get_benchmark_capabilities(
    state: State<'_, AppState>,
) -> Result<Vec<BenchmarkCapability>, AppError> {
    let bench = state.bench();
    let lang = state.language();
    blocking(move || Ok(bench.capabilities(lang))).await
}

/// Starts one benchmark. The controller validates the request and enforces the single
/// active run; the result is persisted and broadcast when it finishes (in any state). Its
/// texts (labels, phases, warnings, notes) are in the language effective at start and are
/// stored that way.
#[tauri::command]
pub async fn start_benchmark(
    app: AppHandle,
    state: State<'_, AppState>,
    request: BenchmarkRequest,
) -> Result<String, AppError> {
    let progress_handle = app.clone();
    let finished_handle = app.clone();
    state.bench().start(
        request,
        state.language(),
        move |progress: &BenchmarkProgress| {
            if let Err(e) = progress_handle.emit(BENCHMARK_PROGRESS_EVENT, progress) {
                tracing::warn!(error = %e, "failed to emit benchmark progress");
            }
        },
        move |result: BenchmarkResult| {
            let state = finished_handle.state::<AppState>();
            match state.db() {
                Ok(db) => {
                    if let Err(e) = db.save_benchmark_result(&result) {
                        tracing::error!(error = %e, run = %result.run_id, "failed to store benchmark result");
                    }
                }
                Err(e) => tracing::error!(error = %e, "database unavailable for benchmark result"),
            }
            if let Err(e) = finished_handle.emit(BENCHMARK_FINISHED_EVENT, &result) {
                tracing::warn!(error = %e, "failed to emit benchmark result");
            }
        },
    )
}

#[tauri::command]
pub async fn cancel_benchmark(state: State<'_, AppState>, run_id: String) -> Result<(), AppError> {
    state.bench().cancel(&run_id)
}

#[tauri::command]
pub async fn get_benchmark_status(
    state: State<'_, AppState>,
) -> Result<Option<BenchmarkProgress>, AppError> {
    Ok(state.bench().status())
}

#[tauri::command]
pub async fn get_benchmark_history(
    state: State<'_, AppState>,
    kind: Option<BenchmarkKind>,
    limit: u32,
) -> Result<Vec<BenchmarkResult>, AppError> {
    Ok(state.db()?.list_benchmark_results(kind, limit)?)
}

/// Compares two stored runs; without `previous_run_id`, the latest earlier comparable run.
#[tauri::command]
pub async fn compare_benchmarks(
    state: State<'_, AppState>,
    current_run_id: String,
    previous_run_id: Option<String>,
) -> Result<BenchmarkComparison, AppError> {
    let db = state.db()?;
    let not_found = |what: &str| AppError::new(ErrorCode::NotFound, format!("{what} not found"));
    let current = db
        .get_benchmark_result(&current_run_id)?
        .ok_or_else(|| not_found("benchmark run"))?;
    let previous = match previous_run_id {
        Some(id) => db
            .get_benchmark_result(&id)?
            .ok_or_else(|| not_found("previous benchmark run"))?,
        None => {
            let history = db.list_benchmark_results(Some(current.kind), 500)?;
            diagnostics::find_previous_comparable(&current, &history)
                .cloned()
                .ok_or_else(|| not_found("earlier comparable run"))?
        }
    };
    Ok(diagnostics::compare_in(
        state.language(),
        &current,
        &previous,
    ))
}
