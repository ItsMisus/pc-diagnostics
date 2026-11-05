//! Process analysis: a sampler thread that enumerates every process, computes per-process
//! rates and the Impact Score, and keeps rankings plus a bounded per-process history.
//! Methodology: docs/impact-score.md.
//!
//! Read-only by construction: the platform layer only enumerates processes and opens
//! `PROCESS_QUERY_LIMITED_INFORMATION` handles to read executable paths. Nothing here
//! terminates, suspends, re-prioritizes or otherwise touches a process.
//!
//! The thread follows the `monitor` pattern: it blocks in `recv_timeout` on a control
//! channel (no busy-wait, prompt stop), uses absolute deadlines, catches panics, and
//! `Drop` stops and joins it.

mod category;
mod key;
mod score;
mod tracker;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use models::process::{ProcessHistory, ProcessSnapshot};
use models::settings::{
    PROCESS_TOP_N_MAX, PROCESS_TOP_N_MIN, SAMPLING_INTERVAL_MAX_MS, SAMPLING_INTERVAL_MIN_MS,
};
use models::{AppError, ErrorCode};
use platform::process::ProcessEnumerator;

pub use category::categorize;
pub use key::ProcessKey;
pub use score::{
    IO_REFERENCE_BYTES_PER_SEC, RAM_AVAILABLE_FLOOR_FRACTION, cpu_percent, cpu_pressure, impact,
    io_pressure, ram_pressure,
};
pub use tracker::{PROCESS_METRIC_SOURCE, ScanInput, Tracker, TrackerConfig};

/// Longest analysis window, seconds.
pub const MAX_WINDOW_S: u32 = 300;
/// Most history points kept per process.
pub const MAX_HISTORY_POINTS: usize = 600;
/// Most process instances tracked at once (running + exited within the window).
pub const MAX_TRACKED_PROCESSES: usize = 2_048;
/// The first ranked snapshot comes at most this long after the baseline scan.
const FIRST_SAMPLE_MAX_DELAY: Duration = Duration::from_millis(1_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessMonitorConfig {
    pub interval_ms: u32,
    pub top_n: u32,
    pub window_s: u32,
    pub history_points: usize,
}

impl Default for ProcessMonitorConfig {
    fn default() -> Self {
        Self {
            interval_ms: 2_000,
            top_n: 10,
            window_s: 60,
            history_points: 150,
        }
    }
}

pub fn validate_top_n(top_n: u32) -> Result<(), AppError> {
    if (PROCESS_TOP_N_MIN..=PROCESS_TOP_N_MAX).contains(&top_n) {
        Ok(())
    } else {
        Err(AppError::invalid_input(format!(
            "topN must be between {PROCESS_TOP_N_MIN} and {PROCESS_TOP_N_MAX}"
        ))
        .with_detail(format!("received {top_n}")))
    }
}

fn validate(config: &ProcessMonitorConfig) -> Result<(), AppError> {
    if !(SAMPLING_INTERVAL_MIN_MS..=SAMPLING_INTERVAL_MAX_MS).contains(&config.interval_ms) {
        return Err(AppError::invalid_input(format!(
            "intervalMs must be between {SAMPLING_INTERVAL_MIN_MS} and {SAMPLING_INTERVAL_MAX_MS}"
        ))
        .with_detail(format!("received {}", config.interval_ms)));
    }
    validate_top_n(config.top_n)?;
    if !(1..=MAX_WINDOW_S).contains(&config.window_s) {
        return Err(AppError::invalid_input(format!(
            "windowS must be between 1 and {MAX_WINDOW_S}"
        ))
        .with_detail(format!("received {}", config.window_s)));
    }
    if !(1..=MAX_HISTORY_POINTS).contains(&config.history_points) {
        return Err(AppError::invalid_input(format!(
            "historyPoints must be between 1 and {MAX_HISTORY_POINTS}"
        ))
        .with_detail(format!("received {}", config.history_points)));
    }
    Ok(())
}

fn next_deadline(scheduled: Instant, interval: Duration, now: Instant) -> Instant {
    let next = scheduled + interval;
    if next <= now { now + interval } else { next }
}

enum Control {
    Stop,
}

struct Shared {
    tracker: Mutex<Tracker>,
    latest: Mutex<Option<ProcessSnapshot>>,
    top_n: AtomicU32,
}

pub struct ProcessMonitor {
    tx: mpsc::Sender<Control>,
    thread: Option<JoinHandle<()>>,
    shared: Arc<Shared>,
}

impl ProcessMonitor {
    /// Starts the sampler thread. The baseline scan runs immediately; the first snapshot
    /// follows `min(interval, 1 s)` later. `on_snapshot` runs on the sampler thread after
    /// each snapshot is stored; keep it short. A panic inside it is caught and logged.
    pub fn start<F>(
        config: ProcessMonitorConfig,
        on_snapshot: F,
    ) -> Result<ProcessMonitor, AppError>
    where
        F: Fn(&ProcessSnapshot) + Send + 'static,
    {
        validate(&config)?;
        let tracker = Tracker::new(TrackerConfig {
            interval_ms: config.interval_ms,
            window_s: config.window_s,
            history_points: config.history_points,
            logical_processors: u32::try_from(platform::overhead::logical_cpu_count())
                .unwrap_or(u32::MAX),
            io_reference_bytes_per_sec: IO_REFERENCE_BYTES_PER_SEC,
            self_pid: std::process::id(),
            system_dir: platform::process::system_directory()
                .map(|d| d.trim_end_matches('\\').to_ascii_lowercase()),
            max_tracked: MAX_TRACKED_PROCESSES,
        });
        let shared = Arc::new(Shared {
            tracker: Mutex::new(tracker),
            latest: Mutex::new(None),
            top_n: AtomicU32::new(config.top_n),
        });
        let (tx, rx) = mpsc::channel();
        let thread_shared = Arc::clone(&shared);
        let interval = Duration::from_millis(u64::from(config.interval_ms));
        let thread = std::thread::Builder::new()
            .name("process-sampler".into())
            .spawn(move || run(&thread_shared, &rx, interval, on_snapshot))
            .map_err(|e| {
                AppError::new(
                    ErrorCode::Internal,
                    "could not spawn process sampler thread",
                )
                .with_detail(e.to_string())
            })?;
        Ok(ProcessMonitor {
            tx,
            thread: Some(thread),
            shared,
        })
    }

    /// Latest snapshot. `NotFound` (retriable) until two scans have completed.
    pub fn snapshot(&self) -> Result<ProcessSnapshot, AppError> {
        self.shared
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| {
                let mut e = AppError::new(
                    ErrorCode::NotFound,
                    "no process snapshot yet (rates need two scans)",
                );
                e.retriable = true;
                e
            })
    }

    /// History of one process instance. `InvalidInput` for a malformed key, `NotFound`
    /// for an unknown one (never seen, or exited more than one window ago).
    pub fn history(&self, key: &str) -> Result<ProcessHistory, AppError> {
        let k: ProcessKey = key.parse()?;
        self.shared
            .tracker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .history(&k)
            .ok_or_else(|| {
                AppError::new(ErrorCode::NotFound, "unknown process key")
                    .with_detail(format!("key {k}"))
            })
    }

    /// Changes the ranking length. Takes effect immediately for `snapshot()` (the stored
    /// snapshot is rebuilt from current state) and for every later event.
    pub fn set_top_n(&self, top_n: u32) -> Result<(), AppError> {
        validate_top_n(top_n)?;
        self.shared.top_n.store(top_n, Ordering::Relaxed);
        let rebuilt = self
            .shared
            .tracker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .snapshot(top_n);
        if let Some(s) = rebuilt {
            *self
                .shared
                .latest
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(s);
        }
        Ok(())
    }

    /// Stops the sampler and joins it.
    pub fn shutdown(self) {
        // `Drop` does the work.
    }

    fn stop_and_join(&mut self) {
        let _ = self.tx.send(Control::Stop);
        if let Some(t) = self.thread.take()
            && t.join().is_err()
        {
            tracing::error!("process sampler thread panicked");
        }
    }
}

impl Drop for ProcessMonitor {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

/// One scan plus window ownership and memory status, then folded into the tracker.
fn tick(
    shared: &Shared,
    enumerator: &mut ProcessEnumerator,
) -> Result<Option<ProcessSnapshot>, AppError> {
    let scan = enumerator.scan()?;
    let windows = platform::process::visible_window_pids()
        .inspect_err(|e| tracing::debug!(error = %e, "window enumeration failed"))
        .ok();
    let memory = platform::process::memory_status()
        .inspect_err(|e| tracing::warn!(error = %e, "memory status failed"))
        .ok();
    let mut tracker = shared
        .tracker
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    tracker.ingest(
        &ScanInput {
            scan: &scan,
            memory,
            window_pids: windows.as_ref(),
        },
        platform::process::image_access,
    );
    Ok(tracker.snapshot(shared.top_n.load(Ordering::Relaxed)))
}

fn run<F>(shared: &Shared, rx: &mpsc::Receiver<Control>, interval: Duration, on_snapshot: F)
where
    F: Fn(&ProcessSnapshot),
{
    let mut enumerator = ProcessEnumerator::new();
    let mut scheduled = Instant::now();
    // Baseline scan right away, the first ranked snapshot shortly after.
    let mut deadline = scheduled;
    let mut first = true;
    loop {
        let wait = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(Control::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        let started = Instant::now();
        match catch_unwind(AssertUnwindSafe(|| tick(shared, &mut enumerator))) {
            Ok(Ok(Some(snap))) => {
                *shared.latest.lock().unwrap_or_else(PoisonError::into_inner) = Some(snap.clone());
                if catch_unwind(AssertUnwindSafe(|| on_snapshot(&snap))).is_err() {
                    tracing::error!("on_snapshot callback panicked");
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(e)) => tracing::warn!(error = %e, "process scan failed; tick skipped"),
            Err(_) => tracing::error!("process tick panicked; tick skipped"),
        }
        tracing::trace!(cost_us = started.elapsed().as_micros(), "process tick");
        scheduled = deadline;
        let step = if first {
            first = false;
            interval.min(FIRST_SAMPLE_MAX_DELAY)
        } else {
            interval
        };
        deadline = next_deadline(scheduled, step, Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn config_validation() {
        assert!(validate(&ProcessMonitorConfig::default()).is_ok());
        let d = ProcessMonitorConfig::default();
        for bad in [
            ProcessMonitorConfig {
                interval_ms: 100,
                ..d
            },
            ProcessMonitorConfig {
                interval_ms: 60_000,
                ..d
            },
            ProcessMonitorConfig { top_n: 0, ..d },
            ProcessMonitorConfig {
                top_n: PROCESS_TOP_N_MAX + 1,
                ..d
            },
            ProcessMonitorConfig { window_s: 0, ..d },
            ProcessMonitorConfig {
                window_s: MAX_WINDOW_S + 1,
                ..d
            },
            ProcessMonitorConfig {
                history_points: 0,
                ..d
            },
            ProcessMonitorConfig {
                history_points: MAX_HISTORY_POINTS + 1,
                ..d
            },
        ] {
            let e = ProcessMonitor::start(bad, |_| {})
                .err()
                .expect("must reject");
            assert_eq!(e.code, ErrorCode::InvalidInput, "{bad:?}");
        }
    }

    #[test]
    fn default_config_matches_contract() {
        let d = ProcessMonitorConfig::default();
        assert_eq!(
            (d.interval_ms, d.top_n, d.window_s, d.history_points),
            (2_000, 10, 60, 150)
        );
    }

    #[test]
    fn monitor_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProcessMonitor>();
    }

    #[test]
    fn deadline_is_absolute_and_skips_missed_ticks() {
        let t0 = Instant::now();
        let i = Duration::from_millis(2_000);
        assert_eq!(next_deadline(t0, i, t0 + Duration::from_millis(30)), t0 + i);
        let late = t0 + Duration::from_millis(4_500);
        assert_eq!(next_deadline(t0, i, late), late + i);
    }

    #[test]
    fn shutdown_is_prompt_with_long_interval_and_snapshot_not_ready() {
        let started = Instant::now();
        let m = ProcessMonitor::start(
            ProcessMonitorConfig {
                interval_ms: SAMPLING_INTERVAL_MAX_MS,
                ..ProcessMonitorConfig::default()
            },
            |_| {},
        )
        .expect("start");
        let e = m.snapshot().expect_err("no snapshot yet");
        assert_eq!(e.code, ErrorCode::NotFound);
        assert!(e.retriable);
        m.shutdown();
        let elapsed = started.elapsed();
        assert!(elapsed < Duration::from_millis(1_000), "took {elapsed:?}");
    }

    #[test]
    fn set_top_n_and_history_validate_input() {
        let m = ProcessMonitor::start(ProcessMonitorConfig::default(), |_| {}).expect("start");
        for bad in [0, PROCESS_TOP_N_MAX + 1] {
            assert_eq!(
                m.set_top_n(bad).expect_err("reject").code,
                ErrorCode::InvalidInput
            );
        }
        assert!(m.set_top_n(PROCESS_TOP_N_MIN).is_ok());
        assert!(m.set_top_n(PROCESS_TOP_N_MAX).is_ok());
        assert_eq!(
            m.history("nope").expect_err("bad").code,
            ErrorCode::InvalidInput
        );
        assert_eq!(
            m.history("1:2").expect_err("unknown").code,
            ErrorCode::NotFound
        );
    }

    // Host smoke test: real enumeration of this machine.
    #[test]
    fn host_snapshot_is_real() {
        let (tx, rx) = channel();
        let m = ProcessMonitor::start(
            ProcessMonitorConfig {
                interval_ms: SAMPLING_INTERVAL_MIN_MS,
                top_n: PROCESS_TOP_N_MAX,
                ..ProcessMonitorConfig::default()
            },
            move |s| {
                let _ = tx.send(s.clone());
            },
        )
        .expect("start");
        let s = rx.recv_timeout(Duration::from_secs(10)).expect("snapshot");
        assert!(s.total_processes > 10, "{}", s.total_processes);
        assert!(s.inaccessible_processes < s.total_processes);
        assert_eq!(s.ranking_instant.len(), PROCESS_TOP_N_MAX as usize);
        assert!(s.context.total_memory_bytes > 0.0);
        assert!(s.context.logical_processors >= 1);
        for r in s.ranking_instant.iter().chain(&s.ranking_window) {
            assert!((0.0..=100.0).contains(&r.pressure.impact));
            assert!(r.pid != 0, "idle excluded");
            assert!(r.impact_peak >= r.pressure.impact - 1e-9);
        }
        assert!(
            s.ranking_instant
                .windows(2)
                .all(|w| w[0].pressure.impact >= w[1].pressure.impact)
        );
        assert!(
            s.ranking_instant
                .iter()
                .any(|r| r.accessible && r.exe_path.is_some())
        );
        // Our own process may not rank, but its history must exist and be flagged.
        m.set_top_n(PROCESS_TOP_N_MAX).expect("top n");
        let me = std::process::id();
        let snap = m.snapshot().expect("stored");
        let tracker = m.shared.tracker.lock().expect("lock");
        let mine = tracker
            .snapshot(u32::MAX)
            .expect("snapshot")
            .ranking_instant
            .into_iter()
            .find(|r| r.pid == me)
            .expect("own pid present");
        drop(tracker);
        assert!(mine.is_self && mine.accessible);
        assert!(mine.working_set_bytes.value().is_some_and(|v| *v > 0.0));
        let h = m.history(&mine.key).expect("history");
        assert!(h.running && !h.points.is_empty());
        assert!(snap.total_processes > 10);
        m.shutdown();
    }
}
