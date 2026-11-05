use models::diagnostics::{DiagnosticReport, ExportedReport, ReportFormat};
use models::{AppError, ErrorCode};
use tauri::{AppHandle, Manager, State};

use super::blocking;
use crate::state::AppState;

const REPORT_DIR_NAME: &str = "PC Diagnostics";
const RECENT_BENCHMARKS: u32 = 50;

/// Runs the deterministic rule engine over everything the app currently knows and stores
/// the report. Inputs that are missing simply reduce coverage; they never fail the run.
#[tauri::command]
pub async fn run_diagnostics(app: AppHandle) -> Result<DiagnosticReport, AppError> {
    blocking(move || {
        let state = app.state::<AppState>();
        let monitor = state.monitor()?;
        let system = platform::system::system_info();
        let inventory = monitor
            .inventory(false)
            .inspect_err(|e| tracing::warn!(error = %e, "inventory unavailable for diagnostics"))
            .ok();
        let live = monitor.history();
        let processes = state.processes()?.snapshot().ok();
        let drivers = driver_inventory_for_report(&state)?;
        let benchmarks = state
            .db()?
            .list_benchmark_results(None, RECENT_BENCHMARKS)?;

        let report = diagnostics::evaluate(&diagnostics::DiagnosticInput {
            app_version: env!("CARGO_PKG_VERSION"),
            now_ms: models::time::now_unix_ms(),
            system: Some(&system),
            inventory: inventory.as_ref(),
            live: &live,
            processes: processes.as_ref(),
            drivers: drivers.as_ref(),
            benchmarks: &benchmarks,
            language: state.language(),
        });
        state.db()?.save_diagnostic_report(&report)?;
        Ok(report)
    })
    .await
}

#[tauri::command]
pub async fn get_latest_diagnostics(
    state: State<'_, AppState>,
) -> Result<Option<DiagnosticReport>, AppError> {
    Ok(state.db()?.latest_diagnostic_report()?)
}

/// Writes the latest report to `Documents\PC Diagnostics`. The path is fixed by the
/// backend; the frontend cannot choose where files are written.
#[tauri::command]
pub async fn export_report(
    app: AppHandle,
    format: ReportFormat,
    redact: bool,
) -> Result<ExportedReport, AppError> {
    let dir = app
        .path()
        .document_dir()
        .map_err(|e| {
            AppError::new(ErrorCode::Platform, "Documents folder not found")
                .with_detail(e.to_string())
        })?
        .join(REPORT_DIR_NAME);

    blocking(move || {
        let state = app.state::<AppState>();
        let report = state
            .db()?
            .latest_diagnostic_report()?
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "no diagnostic report yet"))?;
        let monitor = state.monitor()?;
        let system = platform::system::system_info();
        let inventory = monitor.inventory(false).ok();
        let processes = state.processes()?.snapshot().ok();
        let drivers = driver_inventory_for_report(&state)?;
        let benchmarks = state
            .db()?
            .list_benchmark_results(None, RECENT_BENCHMARKS)?;

        let utc_offset_minutes = platform::system::utc_offset_minutes(report.generated_at);
        let bundle = diagnostics::ReportBundle {
            report: &report,
            system: Some(&system),
            inventory: inventory.as_ref(),
            benchmarks: &benchmarks,
            drivers: drivers.as_ref(),
            processes: processes.as_ref(),
            utc_offset_minutes,
            language: state.language(),
        };
        let content = match format {
            ReportFormat::Json => diagnostics::render_json(&bundle, redact)?,
            ReportFormat::Html => diagnostics::render_html(&bundle, redact)?,
        };

        let io_err = |e: std::io::Error| {
            AppError::new(ErrorCode::Platform, "could not write the report file")
                .with_detail(e.to_string())
        };
        std::fs::create_dir_all(&dir).map_err(io_err)?;
        let path = dir.join(diagnostics::report_file_name(
            &report,
            format,
            utc_offset_minutes,
        ));
        std::fs::write(&path, content.as_bytes()).map_err(io_err)?;
        Ok(ExportedReport {
            path: path.to_string_lossy().into_owned(),
            bytes: content.len() as u64,
            redacted: redact,
        })
    })
    .await
}

/// Cached device inventory, collected now (local only, about 1 s) if the Drivers page was
/// never opened, so diagnostics do not report drivers as "not evaluated" needlessly.
/// A collection failure only reduces coverage.
fn driver_inventory_for_report(
    state: &AppState,
) -> Result<Option<models::drivers::DriverInventory>, AppError> {
    if let Some(cached) = state.driver_inventory()? {
        return Ok(Some(cached));
    }
    match drivers::collect_inventory() {
        Ok(inventory) => {
            state.set_driver_inventory(inventory.clone())?;
            Ok(Some(inventory))
        }
        Err(e) => {
            tracing::warn!(error = %e, "driver inventory unavailable for diagnostics");
            Ok(None)
        }
    }
}
