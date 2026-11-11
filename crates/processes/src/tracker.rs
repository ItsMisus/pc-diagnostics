//! Per-process state across scans: rate computation, Impact Score, rankings and the
//! bounded per-process history. Pure: the OS is reached only through the `ProcessScan`
//! passed in and the `image` callback, so everything is unit-tested with synthetic scans.

use std::collections::{HashMap, HashSet, VecDeque};

use models::metric::{Metric, Source, Unavailability, Unit};
use models::process::{
    IMPACT_METHODOLOGY_VERSION, ImpactWeights, Pressure, ProcessCategory, ProcessContext,
    ProcessHistory, ProcessPoint, ProcessRow, ProcessSnapshot,
};
use models::{AppError, ErrorCode};
use platform::overhead::descendants;
use platform::process::{ImageAccess, MemoryStatus, ProcessScan, RawProcess, filetime_to_unix_ms};

use crate::category::{CategoryInput, categorize};
use crate::key::ProcessKey;
use crate::score::{
    RateGap, cpu_percent, cpu_pressure, io_pressure, mean_and_peak, pressure, ram_pressure,
    rate_per_sec,
};

/// `Source` reported for per-process metrics. The contract has no variant for
/// `NtQuerySystemInformation`; PDH's `Process` object is served from the same kernel
/// snapshot (perfproc), so the values are the ones PDH would report.
#[cfg(windows)]
pub const PROCESS_METRIC_SOURCE: Source = Source::Kernel;
#[cfg(not(windows))]
pub const PROCESS_METRIC_SOURCE: Source = Source::Sysinfo;

/// The System Idle Process: idle time, not load. Never tracked, ranked or counted.
pub const IDLE_PID: u32 = 0;

/// A process first seen after the first scan gets a zero baseline (its counters started
/// inside this interval) only if it was created at most this long before the previous
/// scan; otherwise it was missed and its first rates are unavailable.
const NEW_PROCESS_TOLERANCE_MS: u64 = 1_000;

#[derive(Debug, Clone)]
pub struct TrackerConfig {
    pub interval_ms: u32,
    pub window_s: u32,
    pub history_points: usize,
    pub logical_processors: u32,
    pub io_reference_bytes_per_sec: f64,
    pub self_pid: u32,
    /// `%SystemRoot%\System32`, lower-cased.
    pub system_dir: Option<String>,
    /// Upper bound on tracked process instances (running + recently exited).
    pub max_tracked: usize,
}

/// One scan plus the cheap side information collected with it.
pub struct ScanInput<'a> {
    pub scan: &'a ProcessScan,
    pub memory: Option<MemoryStatus>,
    /// `None` when window enumeration failed this tick.
    pub window_pids: Option<&'a HashSet<u32>>,
}

/// History storage (f32 halves memory; precision is far beyond what is displayed).
#[derive(Debug, Clone, Copy)]
struct Tick {
    at: u64,
    cpu: Option<f32>,
    ws: f32,
    read: Option<f32>,
    write: Option<f32>,
    /// cpu, ram, io, impact pressures.
    p: [f32; 4],
}

#[derive(Debug, Clone, Copy, Default)]
struct Counters {
    cpu_100ns: u64,
    read: u64,
    write: u64,
}

#[derive(Debug, Clone)]
struct Latest {
    at: u64,
    cpu: Result<f64, RateGap>,
    ws: f64,
    private: f64,
    read: Result<f64, RateGap>,
    write: Result<f64, RateGap>,
    pressure: Pressure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Image {
    Accessible(Option<String>),
    Denied,
}

#[derive(Debug)]
struct Tracked {
    key: ProcessKey,
    name: String,
    category: ProcessCategory,
    /// `None` until the path lookup has produced a definite answer.
    image: Option<Image>,
    is_self: bool,
    counters: Counters,
    latest: Option<Latest>,
    ticks: VecDeque<Tick>,
    running: bool,
    exited_at: Option<u64>,
}

impl Tracked {
    fn accessible(&self) -> bool {
        self.image != Some(Image::Denied)
    }

    fn exe_path(&self) -> Option<String> {
        match &self.image {
            Some(Image::Accessible(p)) => p.clone(),
            _ => None,
        }
    }
}

pub struct Tracker {
    cfg: TrackerConfig,
    procs: HashMap<ProcessKey, Tracked>,
    /// Monotonic + wall time of the previous scan.
    prev: Option<(std::time::Instant, u64)>,
    memory: Option<MemoryStatus>,
    first_tick_at: Option<u64>,
    latest_at: Option<u64>,
    /// Processes present in the latest scan but not tracked because of `max_tracked`.
    untracked: u32,
    tick_capacity: usize,
}

fn gap_reason(gap: RateGap) -> Unavailability {
    let e = match gap {
        RateGap::NoBaseline => {
            let mut e = AppError::new(
                ErrorCode::NotFound,
                "no earlier reading for this process yet (rates need two readings)",
            );
            e.retriable = true;
            e
        }
        RateGap::CounterReset => AppError::new(
            ErrorCode::Platform,
            "cumulative counter went backwards; re-baselined",
        ),
        RateGap::EmptyWindow => AppError::new(ErrorCode::Platform, "empty measurement window"),
    };
    Unavailability::Failed(e)
}

fn metric(v: &Result<f64, RateGap>, unit: Unit, at: u64) -> Metric<f64> {
    match v {
        Ok(value) => Metric::Available {
            value: *value,
            unit,
            source: PROCESS_METRIC_SOURCE,
            at,
        },
        Err(gap) => Metric::unavailable(gap_reason(*gap)),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn f32_of(v: f64) -> f32 {
    v as f32
}

#[allow(clippy::cast_precision_loss)]
fn f64_of(v: u64) -> f64 {
    v as f64
}

fn by_impact_then_key(a: (&Tracked, f64), b: (&Tracked, f64)) -> std::cmp::Ordering {
    b.1.total_cmp(&a.1).then_with(|| a.0.key.cmp(&b.0.key))
}

/// Own process plus descendants. A child must not be older than its parent, which
/// rejects stale links to a recycled parent pid.
fn self_tree(self_pid: u32, procs: &[RawProcess]) -> HashSet<u32> {
    let parents: Vec<(u32, u32)> = procs.iter().map(|p| (p.pid, p.parent_pid)).collect();
    let created: HashMap<u32, i64> = procs.iter().map(|p| (p.pid, p.create_time_100ns)).collect();
    let mut set: HashSet<u32> = descendants(self_pid, &parents, |child, parent| {
        match (created.get(&child), created.get(&parent)) {
            (Some(c), Some(p)) => c >= p,
            _ => false,
        }
    })
    .into_iter()
    .collect();
    set.insert(self_pid);
    set
}

impl Tracker {
    pub fn new(cfg: TrackerConfig) -> Self {
        let window_ticks = usize::try_from(
            (u64::from(cfg.window_s) * 1_000).div_ceil(u64::from(cfg.interval_ms.max(1))),
        )
        .unwrap_or(usize::MAX)
        .saturating_add(1);
        let tick_capacity = cfg.history_points.max(window_ticks).max(1);
        Self {
            cfg,
            procs: HashMap::new(),
            prev: None,
            memory: None,
            first_tick_at: None,
            latest_at: None,
            untracked: 0,
            tick_capacity,
        }
    }

    pub fn tracked_count(&self) -> usize {
        self.procs.len()
    }

    fn window_ms(&self) -> u64 {
        u64::from(self.cfg.window_s) * 1_000
    }

    /// Folds one scan into the state. `image(pid, create_time_100ns)` is called once per
    /// new process instance (and again only if it returned `Exited`).
    pub fn ingest(
        &mut self,
        input: &ScanInput<'_>,
        mut image: impl FnMut(u32, i64) -> ImageAccess,
    ) {
        let scan = input.scan;
        if input.memory.is_some() {
            self.memory = input.memory;
        }
        let now = scan.at_unix_ms;
        let prev = self.prev.replace((scan.taken, now));
        let wall = prev.map(|(t, _)| scan.taken.saturating_duration_since(t).as_secs_f64());
        let prev_wall_ms = prev.map(|(_, ms)| ms);
        let self_set = self_tree(self.cfg.self_pid, &scan.processes);
        let (total, avail) = self.memory.map_or((0.0, 0.0), |m| {
            (f64_of(m.total_bytes), f64_of(m.available_bytes))
        });
        let window_ms = self.window_ms();
        // Mark exits first, so the cap can evict them for processes new in this scan.
        let present: HashSet<ProcessKey> = scan
            .processes
            .iter()
            .map(|p| ProcessKey {
                pid: p.pid,
                created_unix_ms: filetime_to_unix_ms(p.create_time_100ns),
            })
            .collect();
        for t in self.procs.values_mut() {
            if t.running && !present.contains(&t.key) {
                t.running = false;
                t.exited_at = Some(now);
            }
        }
        self.prune(now, window_ms);
        self.untracked = 0;

        for raw in &scan.processes {
            if raw.pid == IDLE_PID {
                continue;
            }
            let key = ProcessKey {
                pid: raw.pid,
                created_unix_ms: filetime_to_unix_ms(raw.create_time_100ns),
            };
            let cur = Counters {
                cpu_100ns: raw.cpu_time_100ns,
                read: raw.io_read_bytes,
                write: raw.io_write_bytes,
            };
            let baseline = if let Some(t) = self.procs.get(&key) {
                Some(t.counters)
            } else {
                if !self.make_room() {
                    self.untracked += 1;
                    continue;
                }
                // Started after (or just before) the previous scan: counters began at 0
                // inside this interval, so all of them belong to it.
                let fresh = prev_wall_ms
                    .is_some_and(|p| key.created_unix_ms + NEW_PROCESS_TOLERANCE_MS >= p);
                self.procs.insert(
                    key,
                    Tracked {
                        key,
                        name: raw.name.clone(),
                        category: ProcessCategory::Unknown,
                        image: None,
                        is_self: false,
                        counters: cur,
                        latest: None,
                        ticks: VecDeque::new(),
                        running: true,
                        exited_at: None,
                    },
                );
                fresh.then(Counters::default)
            };
            let Some(t) = self.procs.get_mut(&key) else {
                continue;
            };
            t.running = true;
            t.exited_at = None;
            t.is_self = self_set.contains(&raw.pid);
            if t.image.is_none() {
                t.image = match image(raw.pid, raw.create_time_100ns) {
                    ImageAccess::Accessible { path } => Some(Image::Accessible(path)),
                    ImageAccess::Denied => Some(Image::Denied),
                    ImageAccess::Exited => None,
                };
            }
            let exe = t.exe_path();
            t.category = categorize(
                &CategoryInput {
                    pid: raw.pid,
                    parent_pid: raw.parent_pid,
                    name: &raw.name,
                    session_id: raw.session_id,
                    exe_path: exe.as_deref(),
                    has_visible_window: input.window_pids.map(|s| s.contains(&raw.pid)),
                },
                self.cfg.system_dir.as_deref(),
            );

            if let Some(w) = wall {
                let rate = |f: fn(&Counters) -> u64| {
                    baseline
                        .ok_or(RateGap::NoBaseline)
                        .and_then(|b| rate_per_sec(f(&b), f(&cur), w))
                };
                let cpu = baseline.ok_or(RateGap::NoBaseline).and_then(|b| {
                    cpu_percent(b.cpu_100ns, cur.cpu_100ns, w, self.cfg.logical_processors)
                });
                let read = rate(|c| c.read);
                let write = rate(|c| c.write);
                let ws = f64_of(raw.working_set_bytes);
                let p = pressure(
                    cpu_pressure(cpu.ok()),
                    ram_pressure(Some(ws), total, avail),
                    io_pressure(read.ok(), write.ok(), self.cfg.io_reference_bytes_per_sec),
                    ImpactWeights::DEFAULT,
                );
                t.ticks.push_back(Tick {
                    at: now,
                    cpu: cpu.ok().map(f32_of),
                    ws: f32_of(ws),
                    read: read.ok().map(f32_of),
                    write: write.ok().map(f32_of),
                    p: [p.cpu, p.ram, p.io, p.impact].map(f32_of),
                });
                while t.ticks.len() > self.tick_capacity {
                    t.ticks.pop_front();
                }
                t.latest = Some(Latest {
                    at: now,
                    cpu,
                    ws,
                    private: f64_of(raw.private_bytes),
                    read,
                    write,
                    pressure: p,
                });
            }
            t.counters = cur;
        }

        if wall.is_some() {
            self.latest_at = Some(now);
            self.first_tick_at.get_or_insert(now);
        }
    }

    /// Drops processes that exited more than one window ago.
    fn prune(&mut self, now: u64, window_ms: u64) {
        self.procs.retain(|_, t| {
            t.running
                || t.exited_at
                    .is_none_or(|e| now.saturating_sub(e) <= window_ms)
        });
    }

    /// Ensures there is room for one more instance by evicting the longest-exited one.
    fn make_room(&mut self) -> bool {
        if self.procs.len() < self.cfg.max_tracked {
            return true;
        }
        let oldest = self
            .procs
            .values()
            .filter_map(|t| t.exited_at.map(|e| (e, t.key)))
            .min();
        match oldest {
            Some((_, k)) => {
                self.procs.remove(&k);
                true
            }
            None => false,
        }
    }

    fn row(&self, t: &Tracked, pressure: Pressure, impact_peak: f64) -> Option<ProcessRow> {
        let l = t.latest.as_ref()?;
        let bytes = |v: f64| Metric::Available {
            value: v,
            unit: Unit::Bytes,
            source: PROCESS_METRIC_SOURCE,
            at: l.at,
        };
        Some(ProcessRow {
            key: t.key.to_string(),
            pid: t.key.pid,
            name: t.name.clone(),
            category: t.category,
            accessible: t.accessible(),
            is_self: t.is_self,
            exe_path: t.exe_path(),
            cpu_percent: metric(&l.cpu, Unit::Percent, l.at),
            working_set_bytes: bytes(l.ws),
            private_bytes: bytes(l.private),
            io_read_bytes_per_sec: metric(&l.read, Unit::BytesPerSecond, l.at),
            io_write_bytes_per_sec: metric(&l.write, Unit::BytesPerSecond, l.at),
            pressure,
            impact_peak,
        })
    }

    /// Window statistics of one process: component means and the impact peak.
    fn window_stats(&self, t: &Tracked, latest_at: u64) -> Option<(Pressure, f64)> {
        let window_ms = self.window_ms();
        let in_window: Vec<&Tick> = t
            .ticks
            .iter()
            .filter(|k| k.at + window_ms > latest_at)
            .collect();
        let mean = |i: usize| {
            mean_and_peak(in_window.iter().map(|k| f64::from(k.p[i]))).map_or(0.0, |(m, _)| m)
        };
        let (impact, peak) = mean_and_peak(in_window.iter().map(|k| f64::from(k.p[3])))?;
        Some((
            Pressure {
                cpu: mean(0),
                ram: mean(1),
                io: mean(2),
                impact,
            },
            peak,
        ))
    }

    /// Rankings from the current state. `None` before the second scan.
    pub fn snapshot(&self, top_n: u32) -> Option<ProcessSnapshot> {
        let latest_at = self.latest_at?;
        let n = usize::try_from(top_n).unwrap_or(usize::MAX);
        let running: Vec<&Tracked> = self
            .procs
            .values()
            .filter(|t| t.running && t.latest.as_ref().is_some_and(|l| l.at == latest_at))
            .collect();

        let mut instant: Vec<(&Tracked, f64)> = running
            .iter()
            .filter_map(|t| Some((*t, t.latest.as_ref()?.pressure.impact)))
            .collect();
        instant.sort_by(|a, b| by_impact_then_key(*a, *b));
        let ranking_instant = instant
            .iter()
            .take(n)
            .filter_map(|(t, _)| {
                let p = t.latest.as_ref()?.pressure;
                self.row(t, p, p.impact)
            })
            .collect();

        let mut window: Vec<(&Tracked, f64, Pressure, f64)> = running
            .iter()
            .filter_map(|t| {
                let (p, peak) = self.window_stats(t, latest_at)?;
                Some((*t, p.impact, p, peak))
            })
            .collect();
        window.sort_by(|a, b| by_impact_then_key((a.0, a.1), (b.0, b.1)));
        let ranking_window = window
            .iter()
            .take(n)
            .filter_map(|(t, _, p, peak)| self.row(t, *p, *peak))
            .collect();

        let covered_ms = latest_at
            .saturating_sub(self.first_tick_at.unwrap_or(latest_at))
            .saturating_add(u64::from(self.cfg.interval_ms));
        let window_s = u32::try_from(covered_ms.div_ceil(1_000))
            .unwrap_or(u32::MAX)
            .min(self.cfg.window_s);
        let memory = self.memory.unwrap_or(MemoryStatus {
            total_bytes: 0,
            available_bytes: 0,
        });
        let count = |f: &dyn Fn(&Tracked) -> bool| {
            u32::try_from(running.iter().filter(|t| f(t)).count()).unwrap_or(u32::MAX)
        };
        Some(ProcessSnapshot {
            at: latest_at,
            interval_ms: self.cfg.interval_ms,
            window_s,
            methodology_version: IMPACT_METHODOLOGY_VERSION,
            weights: ImpactWeights::DEFAULT,
            context: ProcessContext {
                logical_processors: self.cfg.logical_processors,
                total_memory_bytes: f64_of(memory.total_bytes),
                available_memory_bytes: f64_of(memory.available_bytes),
                io_reference_bytes_per_sec: self.cfg.io_reference_bytes_per_sec,
            },
            total_processes: count(&|_| true).saturating_add(self.untracked),
            inaccessible_processes: count(&|t| !t.accessible()),
            ranking_instant,
            ranking_window,
        })
    }

    /// Recent history of one process instance (oldest first, at most `history_points`).
    pub fn history(&self, key: &ProcessKey) -> Option<ProcessHistory> {
        let t = self.procs.get(key)?;
        let skip = t.ticks.len().saturating_sub(self.cfg.history_points);
        Some(ProcessHistory {
            key: key.to_string(),
            name: t.name.clone(),
            points: t
                .ticks
                .iter()
                .skip(skip)
                .map(|k| ProcessPoint {
                    at: k.at,
                    cpu_percent: k.cpu.map(f64::from),
                    working_set_bytes: Some(f64::from(k.ws)),
                    io_read_bytes_per_sec: k.read.map(f64::from),
                    io_write_bytes_per_sec: k.write.map(f64::from),
                    impact: f64::from(k.p[3]),
                })
                .collect(),
            running: t.running,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform::process::unix_ms_to_filetime;
    use std::time::{Duration, Instant};

    const GIB: u64 = 1024 * 1024 * 1024;
    const T0: u64 = 1_762_000_000_000;

    fn cfg() -> TrackerConfig {
        TrackerConfig {
            interval_ms: 2_000,
            window_s: 10,
            history_points: 4,
            logical_processors: 8,
            io_reference_bytes_per_sec: 100.0 * 1024.0 * 1024.0,
            self_pid: 500,
            system_dir: Some(r"c:\windows\system32".into()),
            max_tracked: 100,
        }
    }

    fn raw(pid: u32, created_ms: u64, cpu_100ns: u64, ws: u64) -> RawProcess {
        RawProcess {
            pid,
            parent_pid: 1,
            name: format!("p{pid}.exe"),
            session_id: 1,
            create_time_100ns: unix_ms_to_filetime(created_ms),
            cpu_time_100ns: cpu_100ns,
            working_set_bytes: ws,
            private_bytes: ws / 2,
            io_read_bytes: 0,
            io_write_bytes: 0,
        }
    }

    struct Clock {
        base: Instant,
    }

    impl Clock {
        fn scan(&self, secs: u64, processes: Vec<RawProcess>) -> ProcessScan {
            ProcessScan {
                at_unix_ms: T0 + secs * 1_000,
                taken: self.base + Duration::from_secs(secs),
                processes,
            }
        }
    }

    fn mem() -> Option<MemoryStatus> {
        Some(MemoryStatus {
            total_bytes: 16 * GIB,
            available_bytes: 8 * GIB,
        })
    }

    fn feed(t: &mut Tracker, scan: &ProcessScan) {
        feed_with(t, scan, |_, _| ImageAccess::Accessible {
            path: Some(r"C:\Apps\x.exe".into()),
        });
    }

    fn feed_with(t: &mut Tracker, scan: &ProcessScan, image: impl FnMut(u32, i64) -> ImageAccess) {
        let windows = HashSet::from([10]);
        t.ingest(
            &ScanInput {
                scan,
                memory: mem(),
                window_pids: Some(&windows),
            },
            image,
        );
    }

    const SEC: u64 = 10_000_000; // 1 s of CPU in 100 ns units

    #[test]
    fn first_scan_is_baseline_only() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg());
        feed(&mut t, &c.scan(0, vec![raw(10, T0 - 60_000, 0, GIB)]));
        assert!(t.snapshot(10).is_none(), "rates need two readings");
        feed(&mut t, &c.scan(2, vec![raw(10, T0 - 60_000, 4 * SEC, GIB)]));
        let s = t.snapshot(10).expect("snapshot");
        let r = &s.ranking_instant[0];
        // 4 s CPU over 2 s on 8 logical = 25 %.
        assert!((r.cpu_percent.value().copied().expect("cpu") - 25.0).abs() < 1e-9);
        assert_eq!(r.key, format!("10:{}", T0 - 60_000));
        assert_eq!(r.category, ProcessCategory::Application);
        assert!(r.accessible);
        // RAM: 1 GiB / 8 GiB available = 12.5.
        assert!((r.pressure.ram - 12.5).abs() < 1e-9);
        assert!((r.pressure.impact - (0.45 * 25.0 + 0.25 * 12.5)).abs() < 1e-9);
        assert_eq!(s.total_processes, 1);
        assert_eq!(s.methodology_version, IMPACT_METHODOLOGY_VERSION);
        assert_eq!(s.context.logical_processors, 8);
    }

    #[test]
    fn idle_process_is_excluded() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg());
        let idle = |cpu| RawProcess {
            name: String::new(),
            ..raw(0, 0, cpu, 0)
        };
        feed(&mut t, &c.scan(0, vec![idle(0), raw(10, T0 - 1, 0, GIB)]));
        feed(
            &mut t,
            &c.scan(2, vec![idle(16 * SEC), raw(10, T0 - 1, 0, GIB)]),
        );
        let s = t.snapshot(10).expect("snapshot");
        assert_eq!(s.total_processes, 1);
        assert!(s.ranking_instant.iter().all(|r| r.pid != 0));
        assert_eq!(t.tracked_count(), 1);
    }

    #[test]
    fn ranking_is_sorted_with_key_tie_break_and_top_n() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg());
        let procs = |cpu: [u64; 4]| {
            vec![
                raw(30, T0 - 5, cpu[0], GIB),
                raw(9, T0 - 5, cpu[1], GIB),
                raw(20, T0 - 5, cpu[2], GIB),
                raw(10, T0 - 5, cpu[3], GIB),
            ]
        };
        feed(&mut t, &c.scan(0, procs([0; 4])));
        // pid 9 and 30 tie; 20 highest; 10 lowest.
        feed(&mut t, &c.scan(2, procs([2 * SEC, 2 * SEC, 8 * SEC, 0])));
        let s = t.snapshot(3).expect("snapshot");
        let pids: Vec<u32> = s.ranking_instant.iter().map(|r| r.pid).collect();
        assert_eq!(pids, vec![20, 9, 30], "desc, ties by key (numeric pid)");
        let pids: Vec<u32> = s.ranking_window.iter().map(|r| r.pid).collect();
        assert_eq!(pids, vec![20, 9, 30]);
        assert_eq!(t.snapshot(1).expect("s").ranking_instant.len(), 1);
    }

    #[test]
    fn window_mean_peak_and_instant_differ() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(TrackerConfig {
            window_s: 6,
            history_points: 10,
            ..cfg()
        });
        // Process 10: burst then idle. Process 20: steady moderate load.
        let mut cpu10 = 0;
        let mut cpu20 = 0;
        let bursts = [16, 16, 0, 0];
        feed(
            &mut t,
            &c.scan(0, vec![raw(10, T0 - 1, 0, 0), raw(20, T0 - 1, 0, 0)]),
        );
        for (i, b) in bursts.iter().enumerate() {
            cpu10 += b * SEC;
            cpu20 += 4 * SEC;
            let secs = 2 * (i as u64 + 1);
            feed(
                &mut t,
                &c.scan(
                    secs,
                    vec![raw(10, T0 - 1, cpu10, 0), raw(20, T0 - 1, cpu20, 0)],
                ),
            );
        }
        let s = t.snapshot(10).expect("snapshot");
        // Instant: 10 is idle (0), 20 at 25 % CPU.
        assert_eq!(s.ranking_instant[0].pid, 20);
        assert_eq!(
            s.ranking_instant[0].impact_peak,
            s.ranking_instant[0].pressure.impact
        );
        // Window 6 s = last 3 ticks (t=4,6,8): 10 → cpu [100,0,0] mean 33.3; 20 → 25.
        let w10 = s.ranking_window.iter().find(|r| r.pid == 10).expect("10");
        assert!(
            (w10.pressure.cpu - 100.0 / 3.0).abs() < 1e-4,
            "{}",
            w10.pressure.cpu
        );
        assert!((w10.impact_peak - 45.0).abs() < 1e-4, "peak = 0.45·100");
        assert_eq!(s.ranking_window[0].pid, 10);
        assert_eq!(s.window_s, 6);
    }

    #[test]
    fn counter_reset_gives_unavailable_rate_and_rebaselines() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg());
        feed(&mut t, &c.scan(0, vec![raw(10, T0 - 1, 8 * SEC, GIB)]));
        feed(&mut t, &c.scan(2, vec![raw(10, T0 - 1, 2 * SEC, GIB)]));
        let s = t.snapshot(10).expect("s");
        let r = &s.ranking_instant[0];
        assert!(!r.cpu_percent.is_available(), "never a made-up 0");
        assert!(r.working_set_bytes.is_available());
        assert_eq!(r.pressure.cpu, 0.0);
        feed(&mut t, &c.scan(4, vec![raw(10, T0 - 1, 4 * SEC, GIB)]));
        let s = t.snapshot(10).expect("s");
        let v = s.ranking_instant[0]
            .cpu_percent
            .value()
            .copied()
            .expect("cpu");
        assert!(
            (v - 12.5).abs() < 1e-9,
            "re-baselined: 2 s / 2 s / 8 = 12.5 %"
        );
    }

    #[test]
    fn new_process_uses_zero_baseline_missed_one_does_not() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg());
        feed(&mut t, &c.scan(0, vec![raw(10, T0 - 1, 0, GIB)]));
        // 30 started at T0+1 s (inside the interval); 40 is old but was not seen before.
        feed(
            &mut t,
            &c.scan(
                2,
                vec![
                    raw(10, T0 - 1, 0, GIB),
                    raw(30, T0 + 1_000, 2 * SEC, GIB),
                    raw(40, T0 - 3_600_000, 99 * SEC, GIB),
                ],
            ),
        );
        let s = t.snapshot(10).expect("s");
        let get = |pid| {
            s.ranking_instant
                .iter()
                .find(|r| r.pid == pid)
                .expect("row")
        };
        assert!((get(30).cpu_percent.value().copied().expect("cpu") - 12.5).abs() < 1e-9);
        assert!(!get(40).cpu_percent.is_available());
        assert!(get(40).working_set_bytes.is_available());
    }

    #[test]
    fn pid_reuse_creates_a_distinct_instance() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg());
        feed(
            &mut t,
            &c.scan(0, vec![raw(10, T0 - 10_000, 50 * SEC, GIB)]),
        );
        feed(&mut t, &c.scan(2, vec![raw(10, T0 + 1_500, SEC, GIB)]));
        let s = t.snapshot(10).expect("s");
        assert_eq!(s.ranking_instant.len(), 1);
        let r = &s.ranking_instant[0];
        assert_eq!(r.key, format!("10:{}", T0 + 1_500));
        assert!(r.cpu_percent.is_available(), "new instance, zero baseline");
        let old: ProcessKey = format!("10:{}", T0 - 10_000).parse().expect("key");
        assert!(!t.history(&old).expect("kept briefly").running);
    }

    #[test]
    fn history_is_bounded_and_exited_processes_are_pruned() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg()); // window 10 s, history 4 points
        for i in 0..8 {
            feed(
                &mut t,
                &c.scan(
                    2 * i,
                    vec![raw(10, T0 - 1, i * SEC, GIB), raw(20, T0 - 1, 0, GIB)],
                ),
            );
        }
        let key: ProcessKey = format!("10:{}", T0 - 1).parse().expect("key");
        let h = t.history(&key).expect("history");
        assert_eq!(h.points.len(), 4, "at most history_points");
        assert!(
            h.points.windows(2).all(|w| w[0].at < w[1].at),
            "oldest first"
        );
        assert!(h.running);
        assert_eq!(h.points.last().map(|p| p.at), Some(T0 + 14_000));
        // 10 exits at t=16; kept while within the window, pruned after.
        feed(&mut t, &c.scan(16, vec![raw(20, T0 - 1, 0, GIB)]));
        assert!(!t.history(&key).expect("still kept").running);
        let s = t.snapshot(10).expect("s");
        assert!(
            s.ranking_window.iter().all(|r| r.pid != 10),
            "exited not ranked"
        );
        feed(&mut t, &c.scan(26, vec![raw(20, T0 - 1, 0, GIB)]));
        assert!(t.history(&key).is_some(), "exactly one window later: kept");
        feed(&mut t, &c.scan(28, vec![raw(20, T0 - 1, 0, GIB)]));
        assert!(t.history(&key).is_none(), "pruned");
        assert_eq!(t.tracked_count(), 1);
    }

    #[test]
    fn tracked_processes_are_capped() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(TrackerConfig {
            max_tracked: 3,
            ..cfg()
        });
        let many: Vec<RawProcess> = (1..=5).map(|p| raw(p * 10, T0 - 1, 0, GIB)).collect();
        feed(&mut t, &c.scan(0, many.clone()));
        feed(&mut t, &c.scan(2, many.clone()));
        assert_eq!(t.tracked_count(), 3);
        let s = t.snapshot(50).expect("s");
        assert_eq!(s.total_processes, 5, "untracked still counted");
        assert_eq!(s.ranking_instant.len(), 3);
        // Exited instances are evicted first to admit new ones.
        let mut next = vec![raw(99, T0 + 3_000, 0, GIB)];
        next.extend(many[..2].iter().cloned());
        feed(&mut t, &c.scan(4, next));
        assert!(
            t.history(&format!("99:{}", T0 + 3_000).parse().expect("k"))
                .is_some()
        );
        assert!(t.tracked_count() <= 3);
    }

    #[test]
    fn inaccessible_processes_are_listed_and_counted() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg());
        let procs = || vec![raw(10, T0 - 1, 0, GIB), raw(20, T0 - 1, 0, 2 * GIB)];
        let image = |pid: u32, _| {
            if pid == 20 {
                ImageAccess::Denied
            } else {
                ImageAccess::Accessible { path: None }
            }
        };
        feed_with(&mut t, &c.scan(0, procs()), image);
        feed_with(&mut t, &c.scan(2, procs()), image);
        let s = t.snapshot(10).expect("s");
        assert_eq!(s.inaccessible_processes, 1);
        let r = s
            .ranking_instant
            .iter()
            .find(|r| r.pid == 20)
            .expect("listed");
        assert!(!r.accessible);
        assert!(r.exe_path.is_none());
    }

    #[test]
    fn image_lookup_runs_once_per_instance() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg());
        let mut calls = 0;
        for i in 0..4 {
            feed_with(
                &mut t,
                &c.scan(2 * i, vec![raw(10, T0 - 1, 0, GIB)]),
                |_, _| {
                    calls += 1;
                    ImageAccess::Accessible { path: None }
                },
            );
        }
        assert_eq!(calls, 1, "cached per key");
    }

    #[test]
    fn self_and_descendants_are_flagged() {
        let c = Clock {
            base: Instant::now(),
        };
        let mut t = Tracker::new(cfg());
        let mut me = raw(500, T0 - 10_000, 0, GIB);
        me.parent_pid = 1;
        let mut child = raw(501, T0 - 5_000, 0, GIB);
        child.parent_pid = 500;
        // Claims 500 as parent but is older: a stale link to a recycled pid.
        let mut stale = raw(502, T0 - 99_000, 0, GIB);
        stale.parent_pid = 500;
        let procs = vec![me, child, stale, raw(10, T0 - 1, 0, GIB)];
        feed(&mut t, &c.scan(0, procs.clone()));
        feed(&mut t, &c.scan(2, procs));
        let s = t.snapshot(10).expect("s");
        let is_self = |pid| {
            s.ranking_instant
                .iter()
                .find(|r| r.pid == pid)
                .expect("row")
                .is_self
        };
        assert!(is_self(500) && is_self(501));
        assert!(!is_self(502) && !is_self(10));
    }
}
