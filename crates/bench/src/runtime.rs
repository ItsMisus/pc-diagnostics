//! Per-run shared state seen by workloads: the cancellation flag, the progress cell,
//! worker accounting and test hooks.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::{Scope, ScopedJoinHandle};
use std::time::{Duration, Instant};

use models::benchmark_run::{BenchmarkSeries, ResultMetric};
use models::settings::Language;
use models::{AppError, ErrorCode};

/// Test-only knobs. Always present (zero cost), only non-default in unit tests.
#[derive(Debug, Clone, Default)]
pub(crate) struct Hooks {
    /// Panic inside the first CPU worker (exercises the catch_unwind → Failed path).
    pub panic_in_worker: bool,
    /// Lower minimum run duration (seconds) for short tests.
    pub min_duration_s: Option<u32>,
    /// Lower minimum storage file size (MiB) for short tests.
    pub min_file_mib: Option<u32>,
    /// Override of the pre-start background-load observation window.
    pub prep_window: Option<Duration>,
    /// Override of the timed phase length of the random storage test.
    pub storage_phase: Option<Duration>,
    /// Records every applied state transition (target state), in order.
    pub transitions: Option<Arc<Mutex<Vec<models::benchmark::BenchmarkState>>>>,
    /// Override of the watchdog limit (any kind).
    pub hard_limit: Option<Duration>,
    /// GPU: skip the real probe and use this environment.
    pub gpu_env: Option<crate::gpu::GpuEnv>,
    /// GPU: how to start the worker (unit tests use the test binary).
    pub gpu_worker: Option<crate::gpu::WorkerCommand>,
    /// GPU: run a simulated worker mode instead of the real workload.
    pub gpu_simulate: Option<crate::gpu_worker::Simulate>,
    /// GPU: override of the warm-up and of the stall timeout.
    pub gpu_warmup: Option<Duration>,
    pub gpu_stall: Option<Duration>,
    /// GPU: records the pid of every worker process started.
    pub gpu_pids: Option<Arc<Mutex<Vec<u32>>>>,
}

/// Why a workload stopped without a result.
#[derive(Debug)]
pub(crate) enum RunError {
    /// The cancellation flag was observed (user cancel, safety stop or watchdog).
    Cancelled,
    Failed(AppError),
}

impl From<AppError> for RunError {
    fn from(e: AppError) -> Self {
        Self::Failed(e)
    }
}

/// What a completed workload produced.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    pub metrics: Vec<ResultMetric>,
    pub series: Vec<BenchmarkSeries>,
    pub warnings: Vec<String>,
    /// Neutral facts about how the run was performed (e.g. the GPU adapter used).
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProgressCell {
    pub phase: String,
    pub fraction: f64,
    pub interim: Vec<ResultMetric>,
}

pub(crate) struct RunCtx {
    cancel: AtomicBool,
    progress: Mutex<ProgressCell>,
    created: Instant,
    /// Start of the `Running` phase. All series timestamps are relative to it.
    running_since: OnceLock<Instant>,
    pub hooks: Hooks,
    workers: Arc<AtomicUsize>,
    /// Language of every user-facing text of the run (phases, labels, warnings, notes).
    pub lang: Language,
}

impl RunCtx {
    pub fn new(hooks: Hooks, workers: Arc<AtomicUsize>, lang: Language) -> Self {
        Self {
            cancel: AtomicBool::new(false),
            progress: Mutex::new(ProgressCell::default()),
            created: Instant::now(),
            running_since: OnceLock::new(),
            hooks,
            workers,
            lang,
        }
    }

    pub fn mark_running(&self) {
        let _ = self.running_since.set(Instant::now());
    }

    /// Time in the `Running` phase (zero before it).
    pub fn running_elapsed(&self) -> Duration {
        self.running_since
            .get()
            .map_or(Duration::ZERO, Instant::elapsed)
    }

    /// Time since the run was created (preparation included).
    pub fn total_elapsed_ms(&self) -> u64 {
        u64::try_from(self.created.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// `Err(Cancelled)` once the flag is set. Workers call this at fine granularity.
    #[inline]
    pub fn check(&self) -> Result<(), RunError> {
        if self.is_cancelled() {
            Err(RunError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Milliseconds since the `Running` phase started (series timestamps).
    pub fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.running_elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub fn set_phase(&self, phase: &str, fraction: f64) {
        let mut p = self.progress.lock().unwrap_or_else(PoisonError::into_inner);
        if p.phase != phase {
            phase.clone_into(&mut p.phase);
        }
        p.fraction = fraction.clamp(0.0, 1.0);
    }

    pub fn set_fraction(&self, fraction: f64) {
        self.progress
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .fraction = fraction.clamp(0.0, 1.0);
    }

    pub fn set_interim(&self, interim: Vec<ResultMetric>) {
        self.progress
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .interim = interim;
    }

    pub fn progress(&self) -> ProgressCell {
        self.progress
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Spawns a named scoped worker that is counted in the controller's live-worker gauge
    /// for exactly as long as it runs (the guard is dropped on every exit, panics included).
    pub fn spawn_worker<'scope, 'env, T, F>(
        &'env self,
        scope: &'scope Scope<'scope, 'env>,
        name: &str,
        f: F,
    ) -> Result<ScopedJoinHandle<'scope, T>, AppError>
    where
        F: FnOnce() -> T + Send + 'scope,
        T: Send + 'scope,
    {
        let workers = Arc::clone(&self.workers);
        workers.fetch_add(1, Ordering::SeqCst);
        let guard = WorkerGuard(Arc::clone(&workers));
        std::thread::Builder::new()
            .name(name.to_owned())
            .spawn_scoped(scope, move || {
                let _guard = guard;
                f()
            })
            .map_err(|e| {
                AppError::new(
                    ErrorCode::Internal,
                    "could not spawn benchmark worker thread",
                )
                .with_detail(e.to_string())
            })
    }
}

/// Decrements the live-worker gauge on drop. If spawning fails the closure (and the guard
/// inside it) is dropped by `spawn_scoped`, so the gauge stays balanced.
struct WorkerGuard(Arc<AtomicUsize>);

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Joins scoped workers and converts a worker panic into a resumed panic on the driver
/// thread (caught by the controller → `Failed`).
pub(crate) fn join_all<T>(handles: Vec<ScopedJoinHandle<'_, T>>) -> Vec<T> {
    let mut out = Vec::with_capacity(handles.len());
    let mut panic = None;
    for h in handles {
        match h.join() {
            Ok(v) => out.push(v),
            Err(p) => panic = Some(p),
        }
    }
    if let Some(p) = panic {
        std::panic::resume_unwind(p);
    }
    out
}

pub(crate) fn metric(
    id: &str,
    label: &str,
    value: f64,
    unit: models::Unit,
    higher_is_better: bool,
) -> ResultMetric {
    ResultMetric {
        id: id.to_owned(),
        label: label.to_owned(),
        value,
        unit,
        higher_is_better,
    }
}
