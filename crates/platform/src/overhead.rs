//! The app's own resource footprint: this process ("core") and all of its descendant
//! processes ("webview": on Windows these are the WebView2 `msedgewebview2.exe` tree).
//!
//! CPU % is CPU time consumed during the window divided by (window × logical CPUs), so it
//! is a share of the whole machine (0–100). Descendants that exit inside a window are
//! not counted for that window (their final CPU time is no longer readable).
//!
//! Cost: one Toolhelp snapshot (no handles) plus a `sysinfo` refresh restricted to our
//! own pids, so it is cheap enough to run every few seconds on the sampler thread.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use models::metric::{Metric, Source, Unavailability, Unit};
use models::monitor::{AppOverhead, ResourceUsage};
use models::{AppError, ErrorCode};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::convert::cpu_share_percent;

/// All transitive children of `root` in a `(pid, parent)` list. Cycle-safe; `root` itself
/// is excluded. `accept(child, parent)` can veto a link (e.g. a recycled parent pid).
pub fn descendants(
    root: u32,
    parents: &[(u32, u32)],
    accept: impl Fn(u32, u32) -> bool,
) -> Vec<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(pid, parent) in parents {
        if pid != parent && pid != 0 {
            children.entry(parent).or_default().push(pid);
        }
    }
    let mut seen = HashSet::from([root]);
    let mut out = Vec::new();
    let mut queue = VecDeque::from([root]);
    while let Some(p) = queue.pop_front() {
        for &c in children.get(&p).map(Vec::as_slice).unwrap_or_default() {
            if seen.insert(c) && accept(c, p) {
                out.push(c);
                queue.push_back(c);
            }
        }
    }
    out
}

fn no_window_yet() -> Unavailability {
    let mut e = AppError::new(
        ErrorCode::NotFound,
        "No completed measurement window yet (needs two readings)",
    );
    e.retriable = true;
    Unavailability::Failed(e)
}

pub struct OverheadTracker {
    sys: Result<System, Unavailability>,
    self_pid: Option<Pid>,
    logical_cpus: usize,
    /// Accumulated CPU ms per pid at the previous measurement.
    prev_cpu_ms: HashMap<Pid, u64>,
    prev_at: Option<Instant>,
    latest: AppOverhead,
}

impl OverheadTracker {
    /// Creates the tracker and takes the baseline reading.
    pub fn new() -> Self {
        let sys = System::new().map_err(|e| {
            Unavailability::Failed(
                AppError::new(ErrorCode::Platform, "sysinfo init failed")
                    .with_detail(e.to_string()),
            )
        });
        let logical_cpus = logical_cpu_count();
        let mut t = Self {
            sys,
            self_pid: sysinfo::get_current_pid().ok(),
            logical_cpus,
            prev_cpu_ms: HashMap::new(),
            prev_at: None,
            latest: pending_overhead(),
        };
        t.measure();
        t
    }

    pub fn latest(&self) -> &AppOverhead {
        &self.latest
    }

    /// Takes a reading and returns usage over the window since the previous one.
    pub fn measure(&mut self) -> &AppOverhead {
        let now = Instant::now();
        let window_ms = self
            .prev_at
            .map(|p| u64::try_from(now.duration_since(p).as_millis()).unwrap_or(u64::MAX));
        let Some(self_pid) = self.self_pid else {
            let r = Unavailability::Failed(AppError::new(
                ErrorCode::Platform,
                "could not determine own process id",
            ));
            self.latest = AppOverhead {
                core: empty_usage(r.clone()),
                webview: empty_usage(r),
                window_ms: 0,
            };
            return &self.latest;
        };
        let sys = match &mut self.sys {
            Ok(s) => s,
            Err(r) => {
                self.latest = AppOverhead {
                    core: empty_usage(r.clone()),
                    webview: empty_usage(r.clone()),
                    window_ms: 0,
                };
                return &self.latest;
            }
        };

        let candidates = match process_parents(sys) {
            Ok(parents) => descendants(self_pid.as_u32(), &parents, |_, _| true),
            Err(e) => {
                tracing::warn!(error = %e, "process tree snapshot failed");
                Vec::new()
            }
        };
        // Refresh our pids plus last window's pids (so exited ones are dropped from `sys`).
        let mut refresh: Vec<Pid> = candidates.iter().map(|p| Pid::from_u32(*p)).collect();
        refresh.push(self_pid);
        refresh.extend(self.prev_cpu_ms.keys().copied());
        refresh.sort_unstable();
        refresh.dedup();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&refresh),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );

        // Re-validate links with start times: a child cannot start before its parent, which
        // rejects stale links to a recycled parent pid.
        let parents: Vec<(u32, u32)> = candidates
            .iter()
            .filter_map(|p| {
                let proc = sys.process(Pid::from_u32(*p))?;
                Some((*p, proc.parent()?.as_u32()))
            })
            .collect();
        let start = |pid: u32| {
            sys.process(Pid::from_u32(pid))
                .map(sysinfo::Process::start_time)
        };
        let webview: Vec<Pid> = descendants(self_pid.as_u32(), &parents, |child, parent| {
            match (start(child), start(parent)) {
                (Some(c), Some(p)) if c > 0 && p > 0 => c >= p,
                _ => true,
            }
        })
        .into_iter()
        .map(Pid::from_u32)
        .collect();

        let mut next_prev = HashMap::new();
        let mut usage = |pids: &[Pid]| -> ResourceUsage {
            let mut cpu_ms = 0u64;
            let mut ws = 0u64;
            let mut found = 0u32;
            for pid in pids {
                let Some(p) = sys.process(*pid) else { continue };
                found += 1;
                let acc = p.accumulated_cpu_time();
                // A pid new to this window contributes all of its CPU time (it started
                // during the window, or just became our descendant).
                cpu_ms += acc.saturating_sub(self.prev_cpu_ms.get(pid).copied().unwrap_or(0));
                ws += p.memory();
                next_prev.insert(*pid, acc);
            }
            let cpu_percent = match window_ms {
                None => Metric::unavailable(no_window_yet()),
                Some(w) => match cpu_share_percent(cpu_ms, w, self.logical_cpus) {
                    Some(v) => Metric::available(v, Unit::Percent, Source::Sysinfo),
                    None => Metric::unavailable(Unavailability::Failed(AppError::new(
                        ErrorCode::Platform,
                        "empty measurement window or unknown CPU count",
                    ))),
                },
            };
            #[allow(clippy::cast_precision_loss)]
            let working_set_bytes = if found > 0 || pids.is_empty() {
                Metric::available(ws as f64, Unit::Bytes, Source::Sysinfo)
            } else {
                Metric::unavailable(Unavailability::Failed(AppError::new(
                    ErrorCode::NotFound,
                    "process disappeared before it could be read",
                )))
            };
            ResourceUsage {
                cpu_percent,
                working_set_bytes,
                process_count: found,
            }
        };
        let core = usage(&[self_pid]);
        let webview = usage(&webview);
        self.prev_cpu_ms = next_prev;
        self.prev_at = Some(now);
        self.latest = AppOverhead {
            core,
            webview,
            window_ms: window_ms.unwrap_or(0),
        };
        &self.latest
    }
}

impl Default for OverheadTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Placeholder before the first reading: every metric explains that nothing was measured.
pub fn pending_overhead() -> AppOverhead {
    AppOverhead {
        core: empty_usage(no_window_yet()),
        webview: empty_usage(no_window_yet()),
        window_ms: 0,
    }
}

fn empty_usage(r: Unavailability) -> ResourceUsage {
    ResourceUsage {
        cpu_percent: Metric::unavailable(r.clone()),
        working_set_bytes: Metric::unavailable(r),
        process_count: 0,
    }
}

#[cfg(windows)]
fn process_parents(_sys: &mut System) -> Result<Vec<(u32, u32)>, AppError> {
    crate::windows::toolhelp::process_parents()
}

#[cfg(not(windows))]
fn process_parents(sys: &mut System) -> Result<Vec<(u32, u32)>, AppError> {
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    Ok(sys
        .processes()
        .iter()
        .filter_map(|(pid, p)| Some((pid.as_u32(), p.parent()?.as_u32())))
        .collect())
}

/// Logical processors across all processor groups.
pub fn logical_cpu_count() -> usize {
    #[cfg(windows)]
    {
        crate::windows::power::logical_processor_count()
    }
    #[cfg(not(windows))]
    {
        std::thread::available_parallelism().map_or(0, usize::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_children_and_grandchildren_only() {
        let tree = [
            (10, 1),  // root
            (11, 10), // child
            (12, 10), // child
            (13, 11), // grandchild
            (20, 1),  // unrelated
            (21, 20),
            (30, 30), // self-parented (idle-like), ignored
        ];
        let mut d = descendants(10, &tree, |_, _| true);
        d.sort_unstable();
        assert_eq!(d, vec![11, 12, 13]);
        assert!(descendants(99, &tree, |_, _| true).is_empty());
    }

    #[test]
    fn descendants_is_cycle_safe_and_respects_veto() {
        let tree = [(2, 1), (1, 2), (3, 2)];
        let mut d = descendants(1, &tree, |_, _| true);
        d.sort_unstable();
        assert_eq!(d, vec![2, 3]);
        let d = descendants(1, &tree, |child, _| child != 2);
        assert!(d.is_empty(), "vetoed link prunes the subtree");
    }

    #[test]
    fn host_overhead_counts_child_processes() {
        let mut child = if cfg!(windows) {
            std::process::Command::new("ping")
                .args(["-n", "4", "127.0.0.1"])
                .stdout(std::process::Stdio::null())
                .spawn()
        } else {
            std::process::Command::new("sleep").arg("3").spawn()
        }
        .expect("spawn child");
        let mut t = OverheadTracker::new();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let o = t.measure().clone();
        let _ = child.kill();
        let _ = child.wait();
        assert!(o.webview.process_count >= 1, "child must be a descendant");
        assert!(
            o.webview
                .working_set_bytes
                .value()
                .is_some_and(|v| *v > 0.0)
        );
        assert!(
            o.webview
                .cpu_percent
                .value()
                .is_some_and(|v| (0.0..=100.0).contains(v))
        );
    }

    #[test]
    fn host_overhead_measures_own_process() {
        let mut t = OverheadTracker::new();
        assert!(!t.latest().core.cpu_percent.is_available(), "baseline only");
        // Burn a little CPU so the window has something to measure.
        let start = Instant::now();
        let mut x = 0u64;
        while start.elapsed().as_millis() < 300 {
            x = x.wrapping_mul(31).wrapping_add(7);
        }
        std::hint::black_box(x);
        let o = t.measure().clone();
        assert!(o.window_ms >= 300);
        assert_eq!(o.core.process_count, 1);
        let cpu = *o.core.cpu_percent.value().expect("core cpu");
        assert!((0.0..=100.0).contains(&cpu));
        assert!(cpu > 0.0, "busy loop must register");
        assert!(o.core.working_set_bytes.value().is_some_and(|v| *v > 0.0));
    }
}
