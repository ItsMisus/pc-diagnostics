//! Live sampler: a dedicated thread, a bounded ring buffer of `LiveSample`s, the cached
//! hardware inventory and the app's own overhead.
//!
//! The sampler thread blocks on `recv_timeout` of a control channel, so stop and
//! interval changes take effect immediately and there is no busy-waiting. Deadlines are
//! absolute (`scheduled + interval`), so the time spent sampling does not drift the
//! cadence; if a tick is missed entirely it is skipped rather than bursted.

mod ring;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use models::hardware::HardwareInventory;
use models::monitor::{AppOverhead, LiveSample, MonitorStatus};
use models::settings::{SAMPLING_INTERVAL_MAX_MS, SAMPLING_INTERVAL_MIN_MS};
use models::{AppError, ErrorCode};
use platform::live::LiveCollector;
use platform::overhead::{OverheadTracker, pending_overhead};

pub use ring::Ring;

/// Upper bound for `history_capacity` (one day at 1 s).
pub const MAX_HISTORY_CAPACITY: usize = 86_400;
/// Cadence of the app-overhead measurement (inside the sampler thread).
pub const OVERHEAD_WINDOW: Duration = Duration::from_secs(5);
/// The first sample comes at most this long after start (rate counters need a baseline
/// window; waiting a full 10 s interval for the first data point would be pointless).
const FIRST_SAMPLE_MAX_DELAY: Duration = Duration::from_millis(1_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorConfig {
    pub interval_ms: u32,
    pub history_capacity: usize,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1_000,
            history_capacity: 300,
        }
    }
}

/// Validates a sampling interval against the settings bounds.
pub fn validate_interval(interval_ms: u32) -> Result<(), AppError> {
    if (SAMPLING_INTERVAL_MIN_MS..=SAMPLING_INTERVAL_MAX_MS).contains(&interval_ms) {
        Ok(())
    } else {
        Err(AppError::invalid_input(format!(
            "intervalMs must be between {SAMPLING_INTERVAL_MIN_MS} and {SAMPLING_INTERVAL_MAX_MS}"
        ))
        .with_detail(format!("received {interval_ms}")))
    }
}

fn validate_capacity(capacity: usize) -> Result<(), AppError> {
    if (1..=MAX_HISTORY_CAPACITY).contains(&capacity) {
        Ok(())
    } else {
        Err(AppError::invalid_input(format!(
            "historyCapacity must be between 1 and {MAX_HISTORY_CAPACITY}"
        ))
        .with_detail(format!("received {capacity}")))
    }
}

/// Next absolute deadline. A deadline already in the past is skipped (no burst).
fn next_deadline(scheduled: Instant, interval: Duration, now: Instant) -> Instant {
    let next = scheduled + interval;
    if next <= now { now + interval } else { next }
}

enum Control {
    Stop,
    SetInterval(u32),
}

struct Shared {
    history: Mutex<Ring<LiveSample>>,
    interval_ms: AtomicU32,
    samples_collected: AtomicU64,
    overhead: Mutex<AppOverhead>,
    inventory: RwLock<Option<HardwareInventory>>,
    /// Serializes inventory collection so concurrent callers do not run WMI twice.
    inventory_collect: Mutex<()>,
}

pub struct Monitor {
    tx: mpsc::Sender<Control>,
    thread: Option<JoinHandle<()>>,
    shared: Arc<Shared>,
    history_capacity: usize,
}

impl Monitor {
    /// Starts the sampler thread. `on_sample` runs on that thread after each sample has
    /// been pushed into the history; keep it short (e.g. emit an event).
    pub fn start<F>(config: MonitorConfig, on_sample: F) -> Result<Monitor, AppError>
    where
        F: Fn(&LiveSample) + Send + 'static,
    {
        validate_interval(config.interval_ms)?;
        validate_capacity(config.history_capacity)?;
        let shared = Arc::new(Shared {
            history: Mutex::new(Ring::new(config.history_capacity)),
            interval_ms: AtomicU32::new(config.interval_ms),
            samples_collected: AtomicU64::new(0),
            overhead: Mutex::new(pending_overhead()),
            inventory: RwLock::new(None),
            inventory_collect: Mutex::new(()),
        });
        let (tx, rx) = mpsc::channel();
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("monitor-sampler".into())
            .spawn(move || run(&thread_shared, &rx, on_sample))
            .map_err(|e| {
                AppError::new(ErrorCode::Internal, "could not spawn sampler thread")
                    .with_detail(e.to_string())
            })?;
        Ok(Monitor {
            tx,
            thread: Some(thread),
            shared,
            history_capacity: config.history_capacity,
        })
    }

    pub fn set_interval(&self, interval_ms: u32) -> Result<(), AppError> {
        validate_interval(interval_ms)?;
        self.shared
            .interval_ms
            .store(interval_ms, Ordering::Relaxed);
        self.tx
            .send(Control::SetInterval(interval_ms))
            .map_err(|_| AppError::new(ErrorCode::Internal, "sampler thread is not running"))
    }

    /// Samples, oldest first. At most `history_capacity` entries.
    pub fn history(&self) -> Vec<LiveSample> {
        self.shared
            .history
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .to_vec()
    }

    /// Newest sample, if any (cheap: clones one sample). Used by the benchmark safety
    /// controller and for benchmark context.
    pub fn latest(&self) -> Option<LiveSample> {
        self.shared
            .history
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .last()
            .cloned()
    }

    pub fn status(&self) -> MonitorStatus {
        MonitorStatus {
            interval_ms: self.shared.interval_ms.load(Ordering::Relaxed),
            history_capacity: u32::try_from(self.history_capacity).unwrap_or(u32::MAX),
            samples_collected: self.shared.samples_collected.load(Ordering::Relaxed),
        }
    }

    /// Cached hardware inventory; `refresh = true` re-collects. Collection is slow (WMI,
    /// DXGI, NVML): call from a blocking-friendly thread, never from the UI thread.
    pub fn inventory(&self, refresh: bool) -> Result<HardwareInventory, AppError> {
        let cached = || {
            self.shared
                .inventory
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        };
        if !refresh && let Some(inv) = cached() {
            return Ok(inv);
        }
        let _guard = self
            .shared
            .inventory_collect
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // Another caller may have finished collecting while we waited.
        if !refresh && let Some(inv) = cached() {
            return Ok(inv);
        }
        let started = Instant::now();
        let inv = catch_unwind(platform::inventory::collect_inventory)
            .map_err(|_| AppError::internal("hardware inventory collection panicked"))?;
        tracing::info!(
            elapsed_ms = started.elapsed().as_millis(),
            "hardware inventory collected"
        );
        *self
            .shared
            .inventory
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(inv.clone());
        Ok(inv)
    }

    /// Latest app-overhead measurement (refreshed every `OVERHEAD_WINDOW`).
    pub fn app_overhead(&self) -> AppOverhead {
        self.shared
            .overhead
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
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
            tracing::error!("sampler thread panicked");
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

fn run<F>(shared: &Shared, rx: &mpsc::Receiver<Control>, on_sample: F)
where
    F: Fn(&LiveSample),
{
    let init_started = Instant::now();
    let mut collector = LiveCollector::new();
    let mut overhead = OverheadTracker::new();
    *shared
        .overhead
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = overhead.latest().clone();
    tracing::info!(
        elapsed_ms = init_started.elapsed().as_millis(),
        "sampler providers initialized"
    );

    let mut interval = Duration::from_millis(u64::from(shared.interval_ms.load(Ordering::Relaxed)));
    let mut scheduled = Instant::now();
    let mut deadline = scheduled + interval.min(FIRST_SAMPLE_MAX_DELAY);
    let mut last_overhead = Instant::now();
    let mut seq: u64 = 0;

    loop {
        let wait = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(Control::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(Control::SetInterval(ms)) => {
                interval = Duration::from_millis(u64::from(ms));
                deadline = next_deadline(scheduled, interval, Instant::now());
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }

        seq += 1;
        let started = Instant::now();
        match catch_unwind(AssertUnwindSafe(|| collector.sample(seq))) {
            Ok(sample) => {
                shared
                    .history
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(sample.clone());
                shared.samples_collected.fetch_add(1, Ordering::Relaxed);
                if catch_unwind(AssertUnwindSafe(|| on_sample(&sample))).is_err() {
                    tracing::error!(seq, "on_sample callback panicked");
                }
            }
            Err(_) => tracing::error!(seq, "sample collection panicked; tick skipped"),
        }
        tracing::trace!(seq, cost_us = started.elapsed().as_micros(), "sample");

        if last_overhead.elapsed() >= OVERHEAD_WINDOW {
            last_overhead = Instant::now();
            match catch_unwind(AssertUnwindSafe(|| overhead.measure().clone())) {
                Ok(o) => {
                    *shared
                        .overhead
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner) = o
                }
                Err(_) => tracing::error!("app overhead measurement panicked"),
            }
        }

        scheduled = deadline;
        deadline = next_deadline(scheduled, interval, Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn interval_validation_uses_settings_bounds() {
        for ok in [SAMPLING_INTERVAL_MIN_MS, 1_000, SAMPLING_INTERVAL_MAX_MS] {
            assert!(validate_interval(ok).is_ok());
        }
        for bad in [
            0,
            SAMPLING_INTERVAL_MIN_MS - 1,
            SAMPLING_INTERVAL_MAX_MS + 1,
        ] {
            let e = validate_interval(bad).expect_err("must reject");
            assert_eq!(e.code, ErrorCode::InvalidInput);
        }
        assert!(validate_capacity(0).is_err());
        assert!(validate_capacity(MAX_HISTORY_CAPACITY + 1).is_err());
        assert!(validate_capacity(1).is_ok());
    }

    #[test]
    fn deadline_is_absolute_and_skips_missed_ticks() {
        let t0 = Instant::now();
        let i = Duration::from_millis(1_000);
        // Sampling took 30 ms: the next deadline stays on the 1 s grid.
        assert_eq!(next_deadline(t0, i, t0 + Duration::from_millis(30)), t0 + i);
        // We overran the next deadline entirely: skip to now + interval.
        let late = t0 + Duration::from_millis(2_500);
        assert_eq!(next_deadline(t0, i, late), late + i);
    }

    #[test]
    fn start_rejects_invalid_config() {
        let bad = MonitorConfig {
            interval_ms: 100,
            history_capacity: 10,
        };
        let e = Monitor::start(bad, |_| {}).err().expect("must reject");
        assert_eq!(e.code, ErrorCode::InvalidInput);
        let bad = MonitorConfig {
            interval_ms: 1_000,
            history_capacity: 0,
        };
        assert!(Monitor::start(bad, |_| {}).is_err());
    }

    #[test]
    fn monitor_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Monitor>();
    }

    #[test]
    fn shutdown_is_prompt_even_with_long_interval() {
        let started = Instant::now();
        let m = Monitor::start(
            MonitorConfig {
                interval_ms: SAMPLING_INTERVAL_MAX_MS,
                history_capacity: 4,
            },
            |_| {},
        )
        .expect("start");
        m.shutdown();
        let elapsed = started.elapsed();
        assert!(elapsed < Duration::from_millis(1_500), "took {elapsed:?}");
    }

    #[test]
    fn set_interval_validates_and_updates_status() {
        let m = Monitor::start(
            MonitorConfig {
                interval_ms: 2_000,
                history_capacity: 4,
            },
            |_| {},
        )
        .expect("start");
        assert_eq!(
            m.set_interval(10).expect_err("reject").code,
            ErrorCode::InvalidInput
        );
        assert_eq!(m.status().interval_ms, 2_000);
        m.set_interval(750).expect("valid");
        assert_eq!(m.status().interval_ms, 750);
        assert_eq!(m.status().history_capacity, 4);
    }

    // Host smoke test: real sampling at the minimum interval; asserts ordering and bounds.
    #[test]
    fn host_samples_are_ordered_and_bounded() {
        let (tx, rx) = channel();
        let m = Monitor::start(
            MonitorConfig {
                interval_ms: SAMPLING_INTERVAL_MIN_MS,
                history_capacity: 2,
            },
            move |s| {
                let _ = tx.send(s.seq);
            },
        )
        .expect("start");
        let mut seqs = Vec::new();
        for _ in 0..3 {
            seqs.push(rx.recv_timeout(Duration::from_secs(5)).expect("sample"));
        }
        assert_eq!(seqs, vec![1, 2, 3]);
        let h = m.history();
        // The sampler may push between the two reads, so the newest can only be newer.
        assert!(m.latest().map(|s| s.seq) >= h.last().map(|s| s.seq));
        assert!(h.len() <= 2);
        assert!(h.windows(2).all(|w| w[0].seq < w[1].seq), "oldest first");
        assert!(m.status().samples_collected >= 3);
        let last = h.last().expect("history");
        assert!(last.memory.total_bytes.value().is_some_and(|v| *v > 0.0));
        let o = m.app_overhead();
        assert!(o.core.working_set_bytes.value().is_some_and(|v| *v > 0.0));
        m.shutdown();
    }

    #[test]
    fn host_inventory_is_cached() {
        let m = Monitor::start(MonitorConfig::default(), |_| {}).expect("start");
        let a = m.inventory(false).expect("inventory");
        let b = m.inventory(false).expect("cached");
        assert_eq!(a.collected_at, b.collected_at, "served from cache");
        assert!(a.cpu.logical_cores >= 1);
        assert!(a.memory.total_bytes > 0);
    }
}
