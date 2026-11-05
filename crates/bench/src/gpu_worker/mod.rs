//! GPU benchmark worker: the code that runs in the isolated child process.
//!
//! The app executable doubles as the worker. `main` calls [`run_if_requested`] first; when
//! the process was started with [`WORKER_FLAG`] it runs the worker and exits with the
//! returned code, otherwise the app starts normally. The parent side (spawn, watchdog,
//! cancellation, result building) lives in `crate::gpu`.
//!
//! A GPU driver fault (TDR, device removal, driver crash) can take the process down with
//! it; isolating the workload keeps the app alive and turns such an event into a `Failed`
//! run.
//!
//! Worker command line (after the flag; private, same executable on both sides):
//!
//! ```text
//! --probe                         list adapters and exit
//! --duration-ms N --warmup-ms N   timed window and warm-up
//! --adapter-luid luid:H-L         requested GPU (DXGI LUID, optional)
//! --vendor-id N --device-id N     requested GPU PCI ids
//! --exclude-luid luid:H-L         software/virtual adapters (repeatable)
//! --simulate MODE                 test-only fake workloads (no GPU), see `Simulate`
//! ```
//!
//! Exit codes: 0 = done or cancelled, 1 = error reported on stdout, 2 = bad arguments.
//! Anything else (or an exit without a final message) is an abnormal termination.

mod gpu;
pub(crate) mod kernel;
pub(crate) mod protocol;
pub(crate) mod select;

use std::io::{BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use models::AppError;

use protocol::{CANCEL_LINE, Msg, Point, WorkerResult};
use select::Target;

/// First argument that turns the executable into the GPU worker.
pub const WORKER_FLAG: &str = "--pcdiag-gpu-worker";

pub(crate) const EXIT_OK: i32 = 0;
pub(crate) const EXIT_ERROR: i32 = 1;
pub(crate) const EXIT_BAD_ARGS: i32 = 2;

/// Upper bound accepted by the worker itself (the parent enforces the real limits).
const MAX_WORKER_MS: u64 = 300_000;

/// If the process was started as the GPU worker (argv\[1\] is [`WORKER_FLAG`]), runs the
/// worker to completion and returns `Some(exit code)`; otherwise `None` (normal app start).
pub fn run_if_requested() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next()? != WORKER_FLAG {
        return None;
    }
    let rest = args.map(|a| a.to_string_lossy().into_owned()).collect();
    Some(worker_main(rest))
}

/// Test-only fake workloads. They never touch the GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Simulate {
    /// Behaves like a healthy run with constant synthetic throughput.
    Fake,
    /// Aborts after `hello` (like a driver crash taking the process down).
    Crash,
    /// Says `hello`, then hangs silently and ignores cancellation.
    Hang,
    /// Keeps reporting progress forever and ignores cancellation.
    IgnoreCancel,
    /// Exits with code 0 without a final message.
    ExitSilently,
}

impl Simulate {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "fake" => Self::Fake,
            "crash" => Self::Crash,
            "hang" => Self::Hang,
            "ignore-cancel" => Self::IgnoreCancel,
            "exit-silently" => Self::ExitSilently,
            _ => return None,
        })
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Fake => "fake",
            Self::Crash => "crash",
            Self::Hang => "hang",
            Self::IgnoreCancel => "ignore-cancel",
            Self::ExitSilently => "exit-silently",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkerArgs {
    pub probe: bool,
    pub duration_ms: u64,
    pub warmup_ms: u64,
    pub target: Target,
    pub exclude_luids: Vec<String>,
    pub simulate: Option<Simulate>,
}

impl WorkerArgs {
    pub(crate) fn probe() -> Self {
        Self {
            probe: true,
            duration_ms: 0,
            warmup_ms: 0,
            target: Target {
                luid: None,
                vendor_id: 0,
                device_id: 0,
            },
            exclude_luids: Vec::new(),
            simulate: None,
        }
    }

    pub(crate) fn to_args(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.probe {
            v.push("--probe".to_owned());
        }
        v.extend([
            "--duration-ms".to_owned(),
            self.duration_ms.to_string(),
            "--warmup-ms".to_owned(),
            self.warmup_ms.to_string(),
            "--vendor-id".to_owned(),
            self.target.vendor_id.to_string(),
            "--device-id".to_owned(),
            self.target.device_id.to_string(),
        ]);
        if let Some(l) = &self.target.luid {
            v.extend(["--adapter-luid".to_owned(), l.clone()]);
        }
        for l in &self.exclude_luids {
            v.extend(["--exclude-luid".to_owned(), l.clone()]);
        }
        if let Some(s) = self.simulate {
            v.extend(["--simulate".to_owned(), s.as_str().to_owned()]);
        }
        v
    }

    pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
        let mut out = Self::probe();
        out.probe = false;
        let mut it = args.iter();
        while let Some(a) = it.next() {
            let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
            match a.as_str() {
                "--probe" => out.probe = true,
                "--duration-ms" => out.duration_ms = num(value()?)?,
                "--warmup-ms" => out.warmup_ms = num(value()?)?,
                "--vendor-id" => out.target.vendor_id = num(value()?)?,
                "--device-id" => out.target.device_id = num(value()?)?,
                "--adapter-luid" => out.target.luid = Some(luid(value()?)?),
                "--exclude-luid" => out.exclude_luids.push(luid(value()?)?),
                "--simulate" => {
                    let v = value()?;
                    out.simulate =
                        Some(Simulate::parse(v).ok_or_else(|| format!("unknown mode {v}"))?);
                }
                other => return Err(format!("unknown argument {other}")),
            }
        }
        if !out.probe && (out.duration_ms == 0 || out.duration_ms > MAX_WORKER_MS) {
            return Err(format!("--duration-ms must be 1..={MAX_WORKER_MS}"));
        }
        if out.warmup_ms > MAX_WORKER_MS {
            return Err(format!("--warmup-ms must be ≤ {MAX_WORKER_MS}"));
        }
        Ok(out)
    }
}

fn num<T: std::str::FromStr>(s: &str) -> Result<T, String> {
    s.parse().map_err(|_| format!("invalid number {s}"))
}

fn luid(s: &str) -> Result<String, String> {
    let ok = s.strip_prefix("luid:").is_some_and(|r| {
        r.len() == 17
            && r.as_bytes()[8] == b'-'
            && r.bytes()
                .enumerate()
                .all(|(i, c)| i == 8 || c.is_ascii_hexdigit())
    });
    if ok {
        Ok(s.to_owned())
    } else {
        Err(format!("invalid LUID {s}"))
    }
}

/// Writes protocol lines to stdout. A failed write means the parent is gone: the worker
/// then stops (the cancel flag is raised).
struct Out {
    cancel: Arc<AtomicBool>,
}

impl Out {
    fn send(&self, msg: &Msg) {
        let line = protocol::encode(msg);
        let mut out = std::io::stdout().lock();
        if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
            self.cancel.store(true, Ordering::Release);
        }
    }

    fn error(&self, e: &AppError) {
        self.send(&Msg::Error {
            code: e.code,
            message: e.message.clone(),
            detail: e.detail.clone(),
        });
    }
}

/// Raises `cancel` on a `cancel` line or EOF on stdin. Detached: it may stay blocked in
/// `read` until the process exits, which ends it.
fn watch_stdin(cancel: Arc<AtomicBool>) {
    let spawned = std::thread::Builder::new()
        .name("gpu-worker-stdin".into())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut line = String::new();
            loop {
                line.clear();
                match stdin.lock().read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if line.trim() == CANCEL_LINE => break,
                    Ok(_) => {}
                }
            }
            cancel.store(true, Ordering::Release);
        });
    if spawned.is_err() {
        // Without the watcher the parent's kill after the grace period still applies.
        tracing::warn!("could not spawn the stdin watcher");
    }
}

/// Worker entry point. `args` excludes the program name and [`WORKER_FLAG`].
pub fn worker_main(args: Vec<String>) -> i32 {
    let cancel = Arc::new(AtomicBool::new(false));
    let out = Out {
        cancel: Arc::clone(&cancel),
    };
    out.send(&Msg::Hello {
        pid: std::process::id(),
    });
    let parsed = match WorkerArgs::parse(&args) {
        Ok(a) => a,
        Err(e) => {
            out.error(&AppError::invalid_input("invalid GPU worker arguments").with_detail(e));
            return EXIT_BAD_ARGS;
        }
    };
    watch_stdin(Arc::clone(&cancel));
    if let Some(mode) = parsed.simulate {
        return simulate(mode, &parsed, &cancel, &out);
    }
    if parsed.probe {
        out.send(&Msg::Adapters {
            adapters: gpu::probe(),
        });
        return EXIT_OK;
    }
    let params = gpu::RunParams {
        target: parsed.target.clone(),
        exclude_luids: parsed.exclude_luids.clone(),
        warmup: Duration::from_millis(parsed.warmup_ms),
        duration: Duration::from_millis(parsed.duration_ms),
    };
    match gpu::run(&params, &cancel, &mut |m| out.send(m)) {
        Ok(result) => {
            out.send(&Msg::Done { result });
            EXIT_OK
        }
        Err(gpu::WorkErr::Cancelled) => EXIT_OK,
        Err(gpu::WorkErr::Failed(e)) => {
            out.error(&e);
            EXIT_ERROR
        }
    }
}

fn fake_adapter() -> protocol::AdapterReport {
    protocol::AdapterReport {
        name: "Simulated GPU".into(),
        vendor_id: 0x10DE,
        device_id: 0x1,
        backend: "dx12".into(),
        device_type: "discrete".into(),
        luid: Some("luid:00000000-00000001".into()),
        driver: String::new(),
        driver_info: String::new(),
    }
}

/// Synthetic throughput reported by `Simulate::Fake` (FLOP/s).
pub(crate) const FAKE_FLOPS: f64 = 1.0e12;

fn simulate(mode: Simulate, args: &WorkerArgs, cancel: &AtomicBool, out: &Out) -> i32 {
    let tick = Duration::from_millis(20);
    match mode {
        Simulate::Crash => std::process::abort(),
        Simulate::ExitSilently => EXIT_OK,
        Simulate::Hang => loop {
            std::thread::sleep(Duration::from_secs(1));
        },
        Simulate::IgnoreCancel => loop {
            out.send(&Msg::Progress {
                phase: protocol::phase::MEASURE.into(),
                fraction: 0.5,
                flops_per_s: None,
            });
            std::thread::sleep(Duration::from_millis(100));
        },
        Simulate::Fake => {
            out.send(&Msg::Selected {
                adapter: fake_adapter(),
                matched_by: protocol::MatchKind::Luid,
            });
            std::thread::sleep(Duration::from_millis(args.warmup_ms));
            out.send(&Msg::Measuring);
            let total = Duration::from_millis(args.duration_ms);
            let t0 = Instant::now();
            let mut last = Instant::now();
            while t0.elapsed() < total {
                if cancel.load(Ordering::Acquire) {
                    return EXIT_OK;
                }
                if last.elapsed() >= Duration::from_millis(100) {
                    last = Instant::now();
                    out.send(&Msg::Progress {
                        phase: protocol::phase::MEASURE.into(),
                        fraction: t0.elapsed().as_secs_f64() / total.as_secs_f64(),
                        flops_per_s: Some(FAKE_FLOPS),
                    });
                }
                std::thread::sleep(tick);
            }
            let secs = args.duration_ms.div_ceil(1000).max(2);
            let points = (1..=secs)
                .map(|s| Point {
                    elapsed_ms: s * 1000,
                    flops_per_s: FAKE_FLOPS,
                })
                .collect();
            out.send(&Msg::Done {
                result: WorkerResult {
                    flops_per_invocation: kernel::flops_per_invocation(),
                    invocations_per_dispatch: 1024,
                    dispatches_per_batch: 1,
                    batches: secs,
                    total_flops: FAKE_FLOPS * args.duration_ms as f64 / 1000.0,
                    timed_ms: args.duration_ms,
                    warmup_ms: args.warmup_ms,
                    points,
                    verified_elements: 0,
                    max_rel_error: 0.0,
                },
            });
            EXIT_OK
        }
    }
}

/// Unit tests run the worker inside the test binary: the parent spawns the test binary
/// filtered to [`tests::worker_process_entry`] with the worker arguments in this variable.
#[cfg(test)]
pub(crate) const TEST_ARGV_ENV: &str = "PCDIAG_GPU_WORKER_TEST_ARGV";
#[cfg(test)]
pub(crate) const TEST_ARGV_SEP: char = '\u{1f}';

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Worker entry when the test binary is spawned as a worker; a no-op otherwise.
    #[test]
    fn worker_process_entry() {
        if let Ok(argv) = std::env::var(TEST_ARGV_ENV) {
            let args = argv
                .split(TEST_ARGV_SEP)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            std::process::exit(worker_main(args));
        }
    }

    #[test]
    fn args_roundtrip_and_validation() {
        let a = WorkerArgs {
            probe: false,
            duration_ms: 20_000,
            warmup_ms: 2_000,
            target: Target {
                luid: Some("luid:00000000-0000D1B3".into()),
                vendor_id: 0x10DE,
                device_id: 0x2783,
            },
            exclude_luids: vec!["luid:00000000-0000E000".into()],
            simulate: Some(Simulate::IgnoreCancel),
        };
        assert_eq!(WorkerArgs::parse(&a.to_args()), Ok(a.clone()));
        let p = WorkerArgs::probe();
        assert_eq!(WorkerArgs::parse(&p.to_args()), Ok(p));
        let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        assert!(WorkerArgs::parse(&s(&["--duration-ms", "0"])).is_err());
        assert!(WorkerArgs::parse(&s(&["--duration-ms", "999999"])).is_err());
        assert!(WorkerArgs::parse(&s(&["--duration-ms"])).is_err());
        assert!(WorkerArgs::parse(&s(&["--duration-ms", "10", "--adapter-luid", "x"])).is_err());
        assert!(WorkerArgs::parse(&s(&["--duration-ms", "10", "--bogus"])).is_err());
        assert!(WorkerArgs::parse(&s(&["--duration-ms", "10", "--simulate", "nope"])).is_err());
    }

    #[test]
    fn non_worker_start_is_not_hijacked() {
        // The test binary's own argv never starts with the worker flag.
        assert_eq!(run_if_requested(), None);
    }
}
