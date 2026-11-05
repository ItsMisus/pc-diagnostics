//! Benchmark engine (phase 5): a central safety controller and CPU, RAM, storage and GPU
//! workloads. The GPU workload runs in an isolated child process (see [`gpu_worker`]).
//! Methodology, thresholds and their rationale: docs/benchmark-methodology.md.
//!
//! Guarantees:
//! - nothing starts automatically; one heavy benchmark at a time (`Busy` otherwise);
//! - every run is cancellable and stops within ~100 ms (workers poll a shared flag);
//! - every worker thread is joined; buffers and test files are freed/deleted on every exit
//!   path, panics included (a worker panic ends the run as `Failed`);
//! - `on_finished` is called exactly once per started run;
//! - no overclocking, undervolting, BIOS, registry or power-plan changes; no admin rights.
//!
//! ```ignore
//! let latest: bench::LatestSample = Arc::new(move || monitor.latest());
//! let bench = BenchController::new(version, "release".into(), latest);
//! let id = bench.start(request, Language::English, |p| emit(p), |r| emit_finished(r))?;
//! bench.cancel(&id)?;
//! ```

mod context;
mod controller;
mod cpu;
mod gpu;
pub mod gpu_worker;
mod ids;
mod memory;
mod runtime;
mod stats;
mod storage;
mod validate;

#[cfg(test)]
mod controller_tests;

pub use controller::BenchController;

/// Source of the latest live sample for safety checks and context (the shell passes a
/// closure over `Monitor::latest`).
pub type LatestSample =
    std::sync::Arc<dyn Fn() -> Option<models::monitor::LiveSample> + Send + Sync>;

#[cfg(test)]
mod send_sync {
    #[test]
    fn controller_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<super::BenchController>();
    }
}
