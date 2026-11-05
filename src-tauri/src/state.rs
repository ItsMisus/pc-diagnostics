use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use assistant::{ClaudeClient, SessionStore};
use bench::BenchController;

use models::drivers::DriverInventory;
use models::settings::Language;
use models::{AppError, ErrorCode};
use monitor::Monitor;
use persistence::Database;
use processes::ProcessMonitor;

/// Shared application state managed by Tauri.
///
/// `rusqlite::Connection` is not `Sync`, so access is serialized by a mutex. Queries
/// here are tiny; a dedicated writer thread replaces this once history writes arrive.
///
/// Background samplers sit in an `Option` so they can be taken out and stopped on exit;
/// commands hold short-lived `Arc` clones.
pub struct AppState {
    db: Mutex<Database>,
    monitor: Mutex<Option<Arc<Monitor>>>,
    processes: Mutex<Option<Arc<ProcessMonitor>>>,
    drivers: Mutex<Option<DriverInventory>>,
    update_check_running: AtomicBool,
    bench: Arc<BenchController>,
    /// "Risolvi" previews and plans (in memory only; expire on their own).
    assistant_sessions: SessionStore,
    /// Built on first assistant request.
    claude: Mutex<Option<ClaudeClient>>,
    /// Windows display language read once at startup (resolves `LanguagePreference::System`).
    os_is_italian: bool,
}

/// Clears the "update check running" flag when dropped, even on error or panic.
pub struct UpdateCheckGuard<'a>(&'a AtomicBool);

impl Drop for UpdateCheckGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn poisoned(what: &str) -> AppError {
    AppError::internal(format!("{what} lock poisoned by an earlier panic"))
}

fn shutting_down(what: &str) -> AppError {
    AppError::new(ErrorCode::Cancelled, format!("{what} is shutting down"))
}

impl AppState {
    pub fn new(
        db: Database,
        monitor: Arc<Monitor>,
        processes: ProcessMonitor,
        bench: BenchController,
    ) -> Self {
        Self {
            db: Mutex::new(db),
            monitor: Mutex::new(Some(monitor)),
            processes: Mutex::new(Some(Arc::new(processes))),
            drivers: Mutex::new(None),
            update_check_running: AtomicBool::new(false),
            bench: Arc::new(bench),
            assistant_sessions: SessionStore::new(),
            claude: Mutex::new(None),
            os_is_italian: platform::locale::display_language_is_italian(),
        }
    }

    /// Effective language for user-facing text produced by the backend. Falls back to the
    /// Windows display language if the settings cannot be read.
    pub fn language(&self) -> Language {
        let pref = self
            .db()
            .ok()
            .and_then(|db| db.load_settings().ok())
            .map(|s| s.language)
            .unwrap_or_default();
        pref.resolve(self.os_is_italian)
    }

    pub fn db(&self) -> Result<MutexGuard<'_, Database>, AppError> {
        self.db.lock().map_err(|_| poisoned("database"))
    }

    pub fn monitor(&self) -> Result<Arc<Monitor>, AppError> {
        self.monitor
            .lock()
            .map_err(|_| poisoned("monitor"))?
            .clone()
            .ok_or_else(|| shutting_down("monitor"))
    }

    pub fn processes(&self) -> Result<Arc<ProcessMonitor>, AppError> {
        self.processes
            .lock()
            .map_err(|_| poisoned("process monitor"))?
            .clone()
            .ok_or_else(|| shutting_down("process monitor"))
    }

    pub fn bench(&self) -> Arc<BenchController> {
        Arc::clone(&self.bench)
    }

    pub fn assistant_sessions(&self) -> &SessionStore {
        &self.assistant_sessions
    }

    pub fn claude_client(&self) -> Option<ClaudeClient> {
        self.claude.lock().ok().and_then(|c| c.clone())
    }

    /// Stores the client unless another thread got there first; returns the stored one.
    pub fn set_claude_client(&self, client: ClaudeClient) -> ClaudeClient {
        match self.claude.lock() {
            Ok(mut slot) => slot.get_or_insert(client).clone(),
            Err(_) => client,
        }
    }

    pub fn driver_inventory(&self) -> Result<Option<DriverInventory>, AppError> {
        Ok(self
            .drivers
            .lock()
            .map_err(|_| poisoned("drivers"))?
            .clone())
    }

    pub fn set_driver_inventory(&self, inventory: DriverInventory) -> Result<(), AppError> {
        *self.drivers.lock().map_err(|_| poisoned("drivers"))? = Some(inventory);
        Ok(())
    }

    /// Claims the single Windows Update search slot, or fails with `Busy`.
    pub fn begin_update_check(&self) -> Result<UpdateCheckGuard<'_>, AppError> {
        self.update_check_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                AppError::new(ErrorCode::Busy, "a driver update check is already running")
            })?;
        Ok(UpdateCheckGuard(&self.update_check_running))
    }

    /// Removes the samplers; each stops and joins its thread when the last `Arc` drops.
    pub fn shutdown_samplers(&self) {
        let monitor = self.monitor.lock().ok().and_then(|mut m| m.take());
        let processes = self.processes.lock().ok().and_then(|mut p| p.take());
        drop(monitor);
        drop(processes);
    }
}
