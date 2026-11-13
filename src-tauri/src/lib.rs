mod commands;
mod control_store;
mod state;

use std::sync::{Arc, Weak};

use bench::BenchController;
use models::monitor::LIVE_SAMPLE_EVENT;
use models::process::PROCESS_SNAPSHOT_EVENT;
use monitor::{Monitor, MonitorConfig};
use persistence::Database;
use processes::{ProcessMonitor, ProcessMonitorConfig};
use tauri::{Emitter, Manager, RunEvent};

use crate::state::AppState;

const DB_FILE_NAME: &str = "pc-diagnostics.sqlite3";
/// Five minutes at the default 1 s interval. Bounded: memory use is constant.
const HISTORY_CAPACITY: usize = 300;

pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Per-user local data dir (%APPDATA%\<identifier>): no admin rights needed.
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Database::open(&dir.join(DB_FILE_NAME))?;
            tracing::info!(schema = db.schema_version()?, "database ready");
            let settings = db.load_settings()?;

            let handle = app.handle().clone();
            let monitor = Monitor::start(
                MonitorConfig {
                    interval_ms: settings.sampling_interval_ms,
                    history_capacity: HISTORY_CAPACITY,
                },
                move |sample| {
                    if let Err(e) = handle.emit(LIVE_SAMPLE_EVENT, sample) {
                        tracing::warn!(error = %e, "failed to emit live sample");
                    }
                },
            )?;
            let monitor = Arc::new(monitor);
            tracing::info!(
                interval_ms = settings.sampling_interval_ms,
                "monitor started"
            );

            let handle = app.handle().clone();
            let processes = ProcessMonitor::start(
                ProcessMonitorConfig {
                    top_n: settings.process_top_n,
                    ..ProcessMonitorConfig::default()
                },
                move |snapshot| {
                    if let Err(e) = handle.emit(PROCESS_SNAPSHOT_EVENT, snapshot) {
                        tracing::warn!(error = %e, "failed to emit process snapshot");
                    }
                },
            )?;
            tracing::info!(top_n = settings.process_top_n, "process monitor started");

            // Weak: the bench controller must not keep the monitor alive past shutdown.
            let weak: Weak<Monitor> = Arc::downgrade(&monitor);
            let bench = BenchController::new(
                env!("CARGO_PKG_VERSION").to_owned(),
                if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "release"
                }
                .to_owned(),
                Arc::new(move || weak.upgrade().and_then(|m| m.latest())),
            );

            app.manage(AppState::new(db, monitor, processes, bench));

            // Control centers (Performance, Cooling, Lighting, Gaming): each one is managed
            // state of its own, built on a shared document store.
            let store: Arc<dyn control::DocumentStore> = Arc::new(
                control_store::DbDocumentStore::open(&dir.join(DB_FILE_NAME))?,
            );
            // Cooling center: CPU temperature for the curve editor from the live monitor (weak).
            let cooling_monitor = Arc::downgrade(&app.state::<AppState>().monitor()?);
            let cooling = Arc::new(control::cooling::CoolingController::new(
                Arc::new(control::cooling::OsCoolingBackend::new(Arc::new(
                    move || {
                        let sample = cooling_monitor.upgrade()?.latest()?;
                        sample.cpu.temperature_c.value().copied()
                    },
                ))),
                Arc::clone(&store),
            ));
            app.manage(Arc::clone(&cooling));
            let performance = Arc::new(control::performance::PerformanceController::system(
                Arc::clone(&store),
            ));
            performance.resume();
            performance.start_watcher(control::performance::SUPPLY_POLL);
            app.manage(Arc::clone(&performance));

            // Lighting center.
            let lighting = Arc::new(control::lighting::LightingController::system(Arc::clone(
                &store,
            )));
            app.manage(Arc::clone(&lighting));

            // Gaming: library, game profiles and game mode. A game profile links one profile
            // of each center below (`ProfileDomain`).
            let domains: Vec<control::SharedDomain> = vec![performance, cooling, lighting];
            let gaming = Arc::new(control::gaming::GamingController::new(
                Arc::clone(&store),
                domains,
                Box::new(control::gaming::SystemProcessLister::new()),
            ));
            let handle = app.handle().clone();
            gaming.start_watcher(move |event| {
                if let Err(e) = handle.emit(models::control::gaming::GAMING_SESSION_EVENT, event) {
                    tracing::warn!(error = %e, "failed to emit gaming session event");
                }
            });
            app.manage(gaming);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::system::get_system_info,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_language,
            commands::monitor::get_hardware_inventory,
            commands::monitor::get_live_history,
            commands::monitor::get_monitor_status,
            commands::monitor::get_app_overhead,
            commands::processes::get_process_snapshot,
            commands::processes::get_process_history,
            commands::drivers::get_driver_inventory,
            commands::drivers::check_driver_updates,
            commands::bench::get_benchmark_capabilities,
            commands::bench::start_benchmark,
            commands::bench::cancel_benchmark,
            commands::bench::get_benchmark_status,
            commands::bench::get_benchmark_history,
            commands::bench::compare_benchmarks,
            commands::diagnostics::run_diagnostics,
            commands::diagnostics::get_latest_diagnostics,
            commands::diagnostics::export_report,
            commands::assistant::get_assistant_status,
            commands::assistant::set_assistant_api_key,
            commands::assistant::clear_assistant_api_key,
            commands::assistant::preview_fix,
            commands::assistant::request_fix,
            commands::assistant::execute_fix_step,
            commands::cooling::get_cooling_state,
            commands::cooling::apply_cooling_profile,
            commands::cooling::save_cooling_profile,
            commands::cooling::delete_cooling_profile,
            commands::cooling::restore_cooling,
            commands::performance::get_performance_state,
            commands::performance::apply_performance_profile,
            commands::performance::save_performance_profile,
            commands::performance::delete_performance_profile,
            commands::performance::restore_performance_original,
            commands::performance::open_power_settings,
            commands::gaming::get_gaming_state,
            commands::gaming::get_game_art,
            commands::gaming::launch_game,
            commands::gaming::add_manual_game,
            commands::gaming::remove_manual_game,
            commands::gaming::set_game_hidden,
            commands::gaming::set_game_profile,
            commands::gaming::set_game_mode,
            commands::gaming::stop_game_session,
            commands::lighting::lighting_get_state,
            commands::lighting::lighting_refresh,
            commands::lighting::lighting_apply_profile,
            commands::lighting::lighting_apply_setup,
            commands::lighting::lighting_preview_setup,
            commands::lighting::lighting_set_brightness,
            commands::lighting::lighting_save_profile,
            commands::lighting::lighting_delete_profile,
            commands::lighting::lighting_restore,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build PC Diagnostics");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Cancel any benchmark (cleans temp files) and stop sampler threads before exit.
            let state = handle.state::<AppState>();
            state.bench().shutdown();
            state.shutdown_samplers();
            // Gaming: stop the watcher and clear any game override.
            if let Some(gaming) = handle.try_state::<Arc<control::gaming::GamingController>>() {
                gaming.shutdown();
            }
            // Cooling: a game override never outlives the app.
            if let Some(cooling) = handle.try_state::<Arc<control::cooling::CoolingController>>() {
                cooling.shutdown();
            }
            tracing::info!("samplers stopped");
            if let Some(performance) =
                handle.try_state::<Arc<control::performance::PerformanceController>>()
            {
                performance.shutdown();
            }
            // Lighting: stop the effect engine, hand LampArray devices back.
            if let Some(lighting) = handle.try_state::<Arc<control::lighting::LightingController>>()
            {
                lighting.shutdown();
            }
        }
    });
}
