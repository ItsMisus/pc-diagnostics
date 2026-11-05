//! Safety controller: one heavy benchmark at a time, cooperative cancellation, watchdog,
//! safety monitoring, panic containment and exactly-once completion callbacks.
//!
//! Thread layout of one run:
//! - **supervisor** (`bench-supervisor`): owns the callbacks. Collects the context while
//!   `Preparing`, then monitors the run (progress every 250 ms, safety checks, watchdog,
//!   CPU frequency series) and finally builds the result and calls `on_finished`.
//! - **driver** (`bench-driver`, scoped): runs the workload inside `catch_unwind`.
//! - **workers** (`bench-cpu-*`, `bench-mem-*`, `bench-io-*`, scoped inside the driver):
//!   check the cancel flag at fine granularity. `std::thread::scope` guarantees they are
//!   joined before the driver returns, on every path including panics.
//! - **context** (`bench-context`, detached): read-only OS queries (WMI disk model). It is
//!   not joined, so a stalled WMI query can never delay cancellation; it ends on its own.
//! - **GPU worker process** (`GpuCompute` only): the driver thread owns the child process
//!   and its two reader threads (`bench-gpu-stdout`/`-stderr`) and reaps/joins them on
//!   every exit path (see `gpu.rs`).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use models::benchmark::{BenchmarkKind, BenchmarkState};
use models::benchmark_run::{
    BenchmarkCapability, BenchmarkContext, BenchmarkProgress, BenchmarkRequest, BenchmarkResult,
    BenchmarkSeries, ResultMetric, SeriesPoint,
};
use models::settings::Language;
use models::time::now_unix_ms;
use models::{AppError, ErrorCode, Unit};

use crate::LatestSample;
use crate::context::{self, PREP_WINDOW, SampleWindow, StaticFacts};
use crate::gpu::{self, GpuEnv, GpuWatch};
use crate::runtime::{Hooks, Outcome, RunCtx, RunError};
use crate::validate::{self, Env, Plan};
use crate::{cpu, ids, memory, storage};

/// Progress callback cadence.
pub(crate) const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
/// Supervisor polling step (bounds the extra latency of every reaction).
const POLL: Duration = Duration::from_millis(25);
/// Sustained CPU: stop when the OS frequency limit falls below this share of max MHz.
pub(crate) const CPU_LIMIT_STOP_RATIO: f64 = 0.6;
/// …for this many consecutive 1 s checks.
const CPU_LIMIT_STOP_CHECKS: u32 = 2;
const FREQ_CHECK_INTERVAL: Duration = Duration::from_secs(1);

struct ActiveRun {
    id: String,
    plan: Plan,
    state: Mutex<BenchmarkState>,
    ctx: RunCtx,
    safety_stop: Mutex<Option<String>>,
}

impl ActiveRun {
    fn state(&self) -> BenchmarkState {
        *self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Applies a transition only if `BenchmarkState::can_transition_to` allows it.
    fn transition(&self, next: BenchmarkState) -> bool {
        let mut s = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if s.can_transition_to(next) {
            *s = next;
            self.record(next);
            true
        } else {
            false
        }
    }

    fn record(&self, next: BenchmarkState) {
        if let Some(log) = &self.ctx.hooks.transitions {
            log.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(next);
        }
    }

    /// Moves to `Cancelling` (from `Preparing`/`Running`) and raises the cancel flag.
    /// `reason` is recorded as the safety stop (first reason wins). False if not active.
    fn request_cancel(&self, reason: Option<String>) -> bool {
        {
            let mut s = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            match *s {
                BenchmarkState::Preparing | BenchmarkState::Running => {
                    debug_assert!(s.can_transition_to(BenchmarkState::Cancelling));
                    *s = BenchmarkState::Cancelling;
                    self.record(BenchmarkState::Cancelling);
                }
                BenchmarkState::Cancelling => {}
                _ => return false,
            }
        }
        if let Some(r) = reason {
            let mut slot = self
                .safety_stop
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if slot.is_none() {
                tracing::warn!(run_id = %self.id, reason = %r, "benchmark safety stop");
                *slot = Some(r);
            }
        }
        self.ctx.cancel();
        true
    }

    fn progress(&self) -> BenchmarkProgress {
        let p = self.ctx.progress();
        BenchmarkProgress {
            run_id: self.id.clone(),
            kind: self.plan.kind,
            state: self.state(),
            fraction: p.fraction,
            elapsed_ms: self.ctx.total_elapsed_ms(),
            phase: p.phase,
            interim: p.interim,
        }
    }
}

struct Inner {
    app_version: String,
    build_profile: String,
    latest: LatestSample,
    hooks: Hooks,
    logical: u32,
    slot: Mutex<Option<Arc<ActiveRun>>>,
    supervisors: Mutex<Vec<JoinHandle<()>>>,
    shutting_down: AtomicBool,
    workers: Arc<AtomicUsize>,
    /// GPU capability probe, run once on first use (spawns the worker in probe mode).
    gpu_env: OnceLock<GpuEnv>,
}

impl Inner {
    fn gpu_env(&self) -> &GpuEnv {
        self.gpu_env.get_or_init(|| match &self.hooks.gpu_env {
            Some(env) => env.clone(),
            None => gpu::probe_env(match &self.hooks.gpu_worker {
                Some(c) => Ok(c.clone()),
                None => gpu::WorkerCommand::current_exe(),
            }),
        })
    }
}

/// The benchmark engine. `Send + Sync`; share it behind an `Arc` or in managed state.
pub struct BenchController {
    inner: Arc<Inner>,
}

impl BenchController {
    /// Creates the controller and sweeps stale test files left by a crash (only files
    /// matching this engine's exact naming pattern, in its exact locations). Starts nothing.
    pub fn new(app_version: String, build_profile: String, latest: LatestSample) -> Self {
        Self::with_hooks(app_version, build_profile, latest, Hooks::default())
    }

    pub(crate) fn with_hooks(
        app_version: String,
        build_profile: String,
        latest: LatestSample,
        hooks: Hooks,
    ) -> Self {
        let removed = storage::sweep_stale(&storage::sweep_dirs());
        if removed > 0 {
            tracing::info!(removed, "stale benchmark files removed");
        }
        let logical =
            u32::try_from(platform::overhead::logical_cpu_count().max(1)).unwrap_or(u32::MAX);
        Self {
            inner: Arc::new(Inner {
                app_version,
                build_profile,
                latest,
                hooks,
                logical,
                slot: Mutex::new(None),
                supervisors: Mutex::new(Vec::new()),
                shutting_down: AtomicBool::new(false),
                workers: Arc::new(AtomicUsize::new(0)),
                gpu_env: OnceLock::new(),
            }),
        }
    }

    /// One capability per `BenchmarkKind`, texts in `lang`. The first call probes the GPUs
    /// once (starts the worker process in probe mode once for all adapters, typically well
    /// under a second, and reads each physical adapter's thermal thresholds); the result is
    /// cached.
    pub fn capabilities(&self, lang: Language) -> Vec<BenchmarkCapability> {
        let gpu = self.inner.gpu_env();
        validate::ALL_KINDS
            .iter()
            .map(|k| validate::capability(*k, self.inner.logical, Some(gpu), lang))
            .collect()
    }

    /// Validates the request and starts it on a background thread. Returns the run id.
    /// `on_progress` is called about every 250 ms; `on_finished` exactly once. Phases,
    /// metric and series labels, warnings, notes and the safety stop are written in `lang`
    /// (and stored that way with the result).
    pub fn start<P, F>(
        &self,
        request: BenchmarkRequest,
        lang: Language,
        on_progress: P,
        on_finished: F,
    ) -> Result<String, AppError>
    where
        P: Fn(&BenchmarkProgress) + Send + 'static,
        F: FnOnce(BenchmarkResult) + Send + 'static,
    {
        let inner = &self.inner;
        if inner.shutting_down.load(Ordering::Acquire) {
            return Err(AppError::new(
                ErrorCode::Internal,
                "benchmark controller is shutting down",
            ));
        }
        let env = Env {
            logical_processors: inner.logical,
            fixed_volumes: platform::bench_env::fixed_volumes(),
            gpu: (request.kind == BenchmarkKind::GpuCompute).then(|| inner.gpu_env().clone()),
        };
        let plan = validate::resolve(&request, &env, &inner.hooks)?;
        if let Some(vol) = plan.volume.as_deref() {
            let space = platform::bench_env::volume_space(vol)?;
            validate::check_free_space(
                space.free_to_caller_bytes,
                space.total_bytes,
                u64::from(plan.file_mib) * validate::MIB,
            )?;
        }

        let mut slot = inner.slot.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(active) = slot.as_ref() {
            return Err(
                AppError::new(ErrorCode::Busy, "a benchmark is already running")
                    .with_detail(format!("active run {} ({:?})", active.id, active.plan.kind)),
            );
        }
        let run = Arc::new(ActiveRun {
            id: ids::new_run_id(),
            plan,
            state: Mutex::new(BenchmarkState::Idle),
            ctx: RunCtx::new(inner.hooks.clone(), Arc::clone(&inner.workers), lang),
            safety_stop: Mutex::new(None),
        });
        let ok = run.transition(BenchmarkState::Preparing);
        debug_assert!(ok, "Idle -> Preparing");
        run.ctx
            .set_phase(lang.pick("Preparazione", "Preparing"), 0.0);

        let thread_inner = Arc::clone(inner);
        let thread_run = Arc::clone(&run);
        let handle = std::thread::Builder::new()
            .name("bench-supervisor".into())
            .spawn(move || supervise(&thread_inner, &thread_run, on_progress, on_finished))
            .map_err(|e| {
                AppError::new(ErrorCode::Internal, "could not spawn benchmark thread")
                    .with_detail(e.to_string())
            })?;
        *slot = Some(Arc::clone(&run));
        drop(slot);

        let mut sups = inner
            .supervisors
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // Reap finished supervisors so the list stays small.
        let (done, live): (Vec<_>, Vec<_>) = sups.drain(..).partition(JoinHandle::is_finished);
        for h in done {
            let _ = h.join();
        }
        *sups = live;
        sups.push(handle);
        tracing::info!(run_id = %run.id, kind = ?run.plan.kind, "benchmark started");
        Ok(run.id.clone())
    }

    /// Requests cancellation of the active run. `NotFound` if `run_id` is not active.
    pub fn cancel(&self, run_id: &str) -> Result<(), AppError> {
        let slot = self
            .inner
            .slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match slot.as_ref() {
            Some(run) if run.id == run_id && run.request_cancel(None) => {
                tracing::info!(run_id, "benchmark cancel requested");
                Ok(())
            }
            _ => Err(
                AppError::new(ErrorCode::NotFound, "no active benchmark with this id")
                    .with_detail(run_id.to_owned()),
            ),
        }
    }

    /// Progress of the active run, if any.
    pub fn status(&self) -> Option<BenchmarkProgress> {
        self.inner
            .slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|r| r.progress())
    }

    /// Cancels the active run and joins every benchmark thread (app exit). Idempotent.
    /// `on_finished` of the cancelled run is still called (with `Cancelled`) before this
    /// returns.
    pub fn shutdown(&self) {
        self.inner.shutting_down.store(true, Ordering::Release);
        if let Some(run) = self
            .inner
            .slot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            run.request_cancel(None);
        }
        let handles: Vec<_> = self
            .inner
            .supervisors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect();
        for h in handles {
            if h.join().is_err() {
                tracing::error!("benchmark supervisor panicked");
            }
        }
    }

    /// Live benchmark threads (driver + workers). Zero when idle. Diagnostic/testing aid.
    #[doc(hidden)]
    pub fn active_worker_threads(&self) -> usize {
        self.inner.workers.load(Ordering::SeqCst)
    }
}

impl Drop for BenchController {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn execute(plan: &Plan, ctx: &RunCtx, run_id: &str) -> Result<Outcome, RunError> {
    match plan.kind {
        BenchmarkKind::CpuSingleCore
        | BenchmarkKind::CpuMultiCore
        | BenchmarkKind::CpuSustained => cpu::run(
            ctx,
            plan.kind,
            plan.threads,
            plan.duration.unwrap_or_default(),
        ),
        BenchmarkKind::MemoryBandwidth => memory::run_bandwidth(ctx, plan.threads),
        BenchmarkKind::MemoryLatency => memory::run_latency(ctx),
        BenchmarkKind::StorageSequential => storage::run_sequential(ctx, plan, run_id),
        BenchmarkKind::StorageRandom => storage::run_random(ctx, plan, run_id),
        BenchmarkKind::GpuCompute => gpu::run(ctx, plan),
    }
}

fn emit<P: Fn(&BenchmarkProgress)>(on_progress: &P, run: &ActiveRun) {
    let p = run.progress();
    if catch_unwind(AssertUnwindSafe(|| on_progress(&p))).is_err() {
        tracing::error!(run_id = %run.id, "on_progress callback panicked");
    }
}

fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".into())
}

/// Collects the context while `Preparing`. Returns early (with partial facts) on cancel.
fn prepare<P: Fn(&BenchmarkProgress)>(
    inner: &Inner,
    run: &ActiveRun,
    on_progress: &P,
) -> (BenchmarkContext, Vec<String>) {
    let (tx, rx) = mpsc::channel();
    let volume = run.plan.volume.clone();
    let gpu_id = run.plan.request.gpu_id.clone();
    let spawned = std::thread::Builder::new()
        .name("bench-context".into())
        .spawn(move || {
            let _ = tx.send(context::static_facts(volume.as_deref(), gpu_id.as_deref()));
        });
    let mut facts: Option<StaticFacts> = if spawned.is_err() {
        Some(StaticFacts::default())
    } else {
        None
    };
    let mut window = SampleWindow::default();
    window.offer((inner.latest)());
    let observe = window.sampler_seen;
    let prep_window = inner.hooks.prep_window.unwrap_or(PREP_WINDOW);
    let t0 = Instant::now();
    let mut last_emit: Option<Instant> = None;
    loop {
        if run.ctx.is_cancelled() {
            break;
        }
        if facts.is_none() {
            match rx.try_recv() {
                Ok(f) => facts = Some(f),
                Err(mpsc::TryRecvError::Disconnected) => facts = Some(StaticFacts::default()),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if facts.is_some() && (!observe || t0.elapsed() >= prep_window) {
            break;
        }
        if observe {
            window.offer((inner.latest)());
        }
        if last_emit.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL) {
            last_emit = Some(Instant::now());
            emit(on_progress, run);
        }
        std::thread::sleep(POLL);
    }
    context::build(
        run.plan.kind,
        facts.unwrap_or_default(),
        &window,
        inner.logical,
        &inner.build_profile,
        run.ctx.lang,
    )
}

/// Safety checks and context series during `Running`.
struct Safety {
    kind: BenchmarkKind,
    hard_limit: Duration,
    mem_reserve: Option<u64>,
    /// Sustained CPU only: the limit was ≥ the stop ratio at start (a drop is a change).
    watch_limit: bool,
    low_limit_checks: u32,
    last_freq: Option<Instant>,
    last_seq: Option<u64>,
    mhz: Vec<SeriesPoint>,
    limit: Vec<SeriesPoint>,
    /// GpuCompute: temperature stop and GPU telemetry series.
    gpu: Option<GpuWatch>,
}

impl Safety {
    fn new(plan: &Plan, ctx: &BenchmarkContext, hooks: &Hooks, lang: Language) -> Self {
        let kind = plan.kind;
        let is_mem = matches!(
            kind,
            BenchmarkKind::MemoryBandwidth | BenchmarkKind::MemoryLatency
        );
        let start_ratio = match (ctx.cpu_limit_mhz.value(), ctx.cpu_max_mhz.value()) {
            (Some(l), Some(m)) if *m > 0.0 => Some(l / m),
            _ => None,
        };
        Self {
            kind,
            hard_limit: hooks
                .hard_limit
                .unwrap_or_else(|| validate::hard_limit(plan)),
            mem_reserve: is_mem.then(|| memory::memory_reserve(ctx.total_memory_bytes)),
            watch_limit: kind == BenchmarkKind::CpuSustained
                && start_ratio.is_some_and(|r| r >= CPU_LIMIT_STOP_RATIO),
            low_limit_checks: 0,
            last_freq: None,
            last_seq: None,
            mhz: Vec::new(),
            limit: Vec::new(),
            gpu: plan
                .gpu
                .as_ref()
                .map(|g| GpuWatch::new(&g.ready, true, lang)),
        }
    }

    fn is_cpu(&self) -> bool {
        matches!(
            self.kind,
            BenchmarkKind::CpuSingleCore
                | BenchmarkKind::CpuMultiCore
                | BenchmarkKind::CpuSustained
        )
    }

    /// Returns a stop reason (in the run's language), if any. Also records the frequency
    /// series.
    fn tick(&mut self, run: &ActiveRun, latest: &LatestSample) -> Option<String> {
        let lang = run.ctx.lang;
        let elapsed = run.ctx.running_elapsed();
        if elapsed > self.hard_limit {
            let s = self.hard_limit.as_secs();
            return Some(lang.pick(
                format!("Durata massima di sicurezza superata ({s} s): test interrotto."),
                format!("Maximum safe duration exceeded ({s} s): test stopped."),
            ));
        }
        if let Some(reserve) = self.mem_reserve
            && let Ok(m) = platform::process::memory_status()
            && m.available_bytes < reserve
        {
            let (avail, reserve) = (m.available_bytes / validate::MIB, reserve / validate::MIB);
            return Some(lang.pick(
                format!(
                    "Memoria disponibile scesa sotto la soglia di sicurezza ({avail} MiB < \
                     {reserve} MiB): test interrotto."
                ),
                format!(
                    "Available memory fell below the safety threshold ({avail} MiB < \
                     {reserve} MiB): test stopped."
                ),
            ));
        }
        if let Some(w) = self.gpu.as_mut()
            && let Some(reason) = w.tick(run.ctx.elapsed_ms(), latest())
        {
            return Some(reason);
        }
        if !self.is_cpu() {
            return None;
        }
        if let Some(s) = latest()
            && self.last_seq != Some(s.seq)
        {
            self.last_seq = Some(s.seq);
            if let Some(v) = s.cpu.current_mhz.value() {
                self.mhz.push(SeriesPoint {
                    elapsed_ms: run.ctx.elapsed_ms(),
                    value: *v,
                });
            }
        }
        if self
            .last_freq
            .is_some_and(|t| t.elapsed() < FREQ_CHECK_INTERVAL)
        {
            return None;
        }
        self.last_freq = Some(Instant::now());
        let (max, limit) = platform::bench_env::cpu_frequency_limits();
        let (Some(max), Some(limit)) = (max.value().copied(), limit.value().copied()) else {
            return None;
        };
        if self.kind == BenchmarkKind::CpuSustained {
            self.limit.push(SeriesPoint {
                elapsed_ms: run.ctx.elapsed_ms(),
                value: limit,
            });
        }
        if self.watch_limit && max > 0.0 && limit < CPU_LIMIT_STOP_RATIO * max {
            self.low_limit_checks += 1;
            if self.low_limit_checks >= CPU_LIMIT_STOP_CHECKS {
                return Some(lang.pick(
                    format!(
                        "Limite di frequenza CPU del sistema operativo sceso a {limit:.0} MHz \
                         (sotto il 60% del massimo, {max:.0} MHz): test interrotto. La causa \
                         (limitazione termica o energetica) non è determinabile."
                    ),
                    format!(
                        "Operating system CPU frequency limit fell to {limit:.0} MHz (below \
                         60% of the maximum, {max:.0} MHz): test stopped. The cause (thermal \
                         or power limiting) can't be determined."
                    ),
                ));
            }
        } else {
            self.low_limit_checks = 0;
        }
        None
    }

    fn into_series(self, lang: Language) -> Vec<BenchmarkSeries> {
        let mut out = Vec::new();
        if !self.mhz.is_empty() {
            out.push(BenchmarkSeries {
                id: "cpu_mhz".into(),
                label: lang
                    .pick("Frequenza CPU media", "Average CPU frequency")
                    .into(),
                unit: Unit::Megahertz,
                points: self.mhz,
            });
        }
        if !self.limit.is_empty() {
            out.push(BenchmarkSeries {
                id: "limit_mhz".into(),
                label: lang
                    .pick(
                        "Limite di frequenza CPU (sistema operativo)",
                        "CPU frequency limit (operating system)",
                    )
                    .into(),
                unit: Unit::Megahertz,
                points: self.limit,
            });
        }
        if let Some(w) = self.gpu {
            out.extend(w.into_series());
        }
        out
    }
}

/// Drops non-finite metric values and series points (NaN/inf break JSON reports) and
/// empty series.
pub(crate) fn sanitize(metrics: &mut Vec<ResultMetric>, series: &mut Vec<BenchmarkSeries>) {
    metrics.retain(|m| {
        let ok = m.value.is_finite();
        if !ok {
            tracing::warn!(id = %m.id, "dropping non-finite benchmark metric");
        }
        ok
    });
    for s in series.iter_mut() {
        s.points.retain(|p| p.value.is_finite());
    }
    series.retain(|s| !s.points.is_empty());
}

enum Exec {
    Done(Result<Outcome, RunError>),
    Panicked(String),
}

/// Runs the workload on a scoped driver thread and monitors it until it ends.
fn run_monitored<P: Fn(&BenchmarkProgress)>(
    inner: &Inner,
    run: &ActiveRun,
    on_progress: &P,
    safety: &mut Safety,
) -> Exec {
    std::thread::scope(|s| {
        let driver = run.ctx.spawn_worker(s, "bench-driver", || {
            catch_unwind(AssertUnwindSafe(|| execute(&run.plan, &run.ctx, &run.id)))
        });
        let driver = match driver {
            Ok(h) => h,
            Err(e) => return Exec::Done(Err(RunError::Failed(e))),
        };
        let mut last_emit = Instant::now();
        emit(on_progress, run);
        while !driver.is_finished() {
            std::thread::sleep(POLL);
            if last_emit.elapsed() >= PROGRESS_INTERVAL {
                last_emit = Instant::now();
                if let Some(reason) = safety.tick(run, &inner.latest) {
                    run.request_cancel(Some(reason));
                }
                emit(on_progress, run);
            }
        }
        match driver.join() {
            Ok(Ok(r)) => Exec::Done(r),
            Ok(Err(p)) | Err(p) => Exec::Panicked(panic_message(p.as_ref())),
        }
    })
}

fn supervise<P, F>(inner: &Inner, run: &Arc<ActiveRun>, on_progress: P, on_finished: F)
where
    P: Fn(&BenchmarkProgress),
    F: FnOnce(BenchmarkResult),
{
    let started_at = now_unix_ms();
    let t0 = Instant::now();
    let (context, mut warnings) = prepare(inner, run, &on_progress);
    if let Some(w) = run
        .plan
        .gpu
        .as_ref()
        .and_then(|g| g.ready.thermal.start_warning(run.ctx.lang))
    {
        warnings.push(w.to_owned());
    }

    let mut safety = Safety::new(&run.plan, &context, &run.ctx.hooks, run.ctx.lang);
    let exec = if !run.ctx.is_cancelled() && run.transition(BenchmarkState::Running) {
        run.ctx.mark_running();
        run.ctx
            .set_phase(run.ctx.lang.pick("Avvio", "Starting"), 0.0);
        run_monitored(inner, run, &on_progress, &mut safety)
    } else {
        Exec::Done(Err(RunError::Cancelled))
    };

    let mut series = Vec::new();
    let mut metrics = Vec::new();
    let mut notes = Vec::new();
    let mut error = None;
    let state = match exec {
        Exec::Panicked(msg) => {
            tracing::error!(run_id = %run.id, panic = %msg, "benchmark worker panicked");
            error = Some(
                AppError::internal("a benchmark worker thread panicked; the run was stopped")
                    .with_detail(msg),
            );
            BenchmarkState::Failed
        }
        // Atomic: a cancel that lands first (state already Cancelling) wins.
        Exec::Done(Ok(out)) if run.transition(BenchmarkState::Completed) => {
            metrics = out.metrics;
            series = out.series;
            warnings.extend(out.warnings);
            notes = out.notes;
            BenchmarkState::Completed
        }
        Exec::Done(Ok(_) | Err(RunError::Cancelled)) => BenchmarkState::Cancelled,
        Exec::Done(Err(RunError::Failed(e))) => {
            if run.state() == BenchmarkState::Cancelling {
                // A cancel was requested; errors raised while stopping are not failures.
                BenchmarkState::Cancelled
            } else {
                error = Some(e);
                BenchmarkState::Failed
            }
        }
    };
    if state == BenchmarkState::Cancelled && run.state() != BenchmarkState::Cancelling {
        // Cancelled without a cancel request (e.g. a workload saw the flag set by a
        // safety stop that raced): pass through Cancelling as the state machine requires.
        run.transition(BenchmarkState::Cancelling);
    }
    if state != BenchmarkState::Completed {
        let applied = run.transition(state);
        debug_assert!(applied, "illegal final transition to {state:?}");
    }
    series.extend(safety.into_series(run.ctx.lang));
    sanitize(&mut metrics, &mut series);

    let safety_stop = run
        .safety_stop
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let result = BenchmarkResult {
        run_id: run.id.clone(),
        kind: run.plan.kind,
        state,
        methodology_version: validate::methodology_version(run.plan.kind),
        app_version: inner.app_version.clone(),
        build_profile: inner.build_profile.clone(),
        started_at,
        finished_at: now_unix_ms(),
        duration_ms: u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX),
        request: run.plan.request.clone(),
        context,
        metrics,
        series,
        warnings,
        notes,
        safety_stop,
        error,
    };
    tracing::info!(run_id = %run.id, state = ?state, duration_ms = result.duration_ms, "benchmark finished");

    // Free the slot before notifying, so a caller may start the next run from the callback.
    {
        let mut slot = inner.slot.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.as_ref().is_some_and(|r| Arc::ptr_eq(r, run)) {
            *slot = None;
        }
    }
    if catch_unwind(AssertUnwindSafe(|| on_finished(result))).is_err() {
        tracing::error!(run_id = %run.id, "on_finished callback panicked");
    }
}
