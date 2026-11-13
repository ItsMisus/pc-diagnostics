//! GPU compute benchmark, parent side: capability probe, worker process lifecycle
//! (spawn, protocol, cancellation, stall detection, kill), and result building.
//! The workload itself runs in the child (`crate::gpu_worker`).
//!
//! Process rules:
//! - the worker is this executable started with `--pcdiag-gpu-worker`, no console window,
//!   stdin/stdout/stderr piped, and (Windows) placed in a kill-on-close job object. macOS
//!   has no job objects: the worker stops by itself when its parent is gone
//!   (`gpu_worker::watch_parent`), and every exit path of this module still kills and
//!   reaps it;
//! - stop = write `cancel` + close stdin, wait up to [`CANCEL_GRACE`], then kill;
//! - every exit path waits for the process and joins both reader threads (no zombies);
//! - a worker that dies, is killed, goes silent for [`STALL_TIMEOUT`] or exits without a
//!   final message fails the run with [`ABNORMAL_EXIT`].

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use models::benchmark_run::{BenchmarkSeries, SeriesPoint};
use models::hardware::GpuInfo;
use models::metric::Unavailability;
use models::settings::Language;
use models::{AppError, ErrorCode, Unit};
use platform::gpu_bench::{GpuThermalThresholds, KillOnCloseJob};

use crate::gpu_worker::protocol::{self, AdapterReport, MatchKind, Msg, WorkerResult};
use crate::gpu_worker::select::{self, COMPUTE_API, NoMatch, Target};
use crate::gpu_worker::{Simulate, WORKER_FLAG, WorkerArgs, kernel};
use crate::runtime::{Outcome, RunCtx, RunError, metric};
use crate::stats;
use crate::validate::Plan;

/// `AppError` message (technical, English): the UI shows a translated title per code.
pub(crate) const ABNORMAL_EXIT: &str =
    "the GPU benchmark process ended abnormally (possible driver reset)";

pub(crate) fn warn_gpu_temperature(lang: Language) -> &'static str {
    lang.pick(
        "Temperatura GPU non osservabile: applicati limiti di durata conservativi",
        "GPU temperature not observable: conservative duration limits applied",
    )
}

pub(crate) fn warn_gpu_no_threshold(lang: Language) -> &'static str {
    lang.pick(
        "Soglia termica non fornita dal driver: applicati limiti di durata conservativi",
        "No thermal threshold provided by the driver: conservative duration limits applied",
    )
}

/// Untimed warm-up (clock ramp-up and dispatch calibration) before the timed window.
pub(crate) const WARMUP: Duration = Duration::from_secs(2);
/// After `cancel` + stdin close the worker gets this long before it is killed.
pub(crate) const CANCEL_GRACE: Duration = Duration::from_secs(1);
/// After the final message the worker gets this long to tear down before it is killed.
const EXIT_GRACE: Duration = Duration::from_secs(3);
/// A worker that sends nothing for this long is considered hung (it reports progress every
/// 250 ms; device creation and shader compilation take well under a few seconds).
pub(crate) const STALL_TIMEOUT: Duration = Duration::from_secs(20);
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);
/// Stop when the GPU comes within this margin of the NVML slowdown threshold.
pub(crate) const TEMP_MARGIN_C: f64 = 5.0;
/// Maximum timed window with an observable temperature and known threshold…
pub(crate) const MAX_S_OBSERVED: u32 = 60;
/// …and without (conservative cap).
pub(crate) const MAX_S_UNOBSERVED: u32 = 30;
pub(crate) const DEFAULT_S: u32 = 20;
pub(crate) const MIN_S: u32 = 5;
/// Consecutive 1 s checks without a temperature before an observed run is stopped.
pub(crate) const TEMP_LOST_CHECKS: u32 = 3;
const STDERR_TAIL: usize = 2048;
/// `CREATE_NO_WINDOW`: the worker never opens a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// How to start the worker. Production: this executable + [`WORKER_FLAG`]. Unit tests:
/// the test binary filtered to the worker entry test, arguments in an env variable.
#[derive(Debug, Clone)]
pub(crate) struct WorkerCommand {
    pub program: PathBuf,
    pub prefix_args: Vec<String>,
    /// If set, worker arguments go into this env variable (joined by U+001F) instead of
    /// the command line.
    pub args_env: Option<String>,
}

impl WorkerCommand {
    pub fn current_exe() -> Result<Self, AppError> {
        let program = std::env::current_exe().map_err(|e| {
            AppError::new(ErrorCode::Platform, "cannot locate the app executable")
                .with_detail(e.to_string())
        })?;
        Ok(Self {
            program,
            prefix_args: vec![WORKER_FLAG.to_owned()],
            args_env: None,
        })
    }

    #[cfg(test)]
    pub fn test_binary() -> Self {
        Self {
            program: std::env::current_exe().expect("test exe"),
            prefix_args: [
                "--exact",
                "gpu_worker::tests::worker_process_entry",
                "--nocapture",
                "--test-threads=1",
                "--quiet",
            ]
            .map(str::to_owned)
            .to_vec(),
            args_env: Some(crate::gpu_worker::TEST_ARGV_ENV.to_owned()),
        }
    }

    fn command(&self, args: &[String]) -> Command {
        let mut c = Command::new(&self.program);
        c.args(&self.prefix_args);
        match &self.args_env {
            Some(var) => {
                let joined: String = args.join("\u{1f}");
                c.env(var, OsString::from(joined));
            }
            None => {
                c.args(args);
            }
        }
        c.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(CREATE_NO_WINDOW);
        }
        c
    }
}

enum Line {
    Msg(Msg),
    Bad(String),
    Eof,
}

/// A running worker. Dropping it kills and reaps the process and joins its readers.
struct WorkerProcess {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    rx: mpsc::Receiver<Line>,
    readers: Vec<JoinHandle<()>>,
    stderr_tail: Arc<Mutex<String>>,
    _job: Option<KillOnCloseJob>,
}

impl WorkerProcess {
    fn spawn(cmd: &WorkerCommand, args: &[String]) -> Result<Self, AppError> {
        let mut child = cmd.command(args).spawn().map_err(|e| {
            AppError::new(
                ErrorCode::Platform,
                "could not start the GPU benchmark process",
            )
            .with_detail(format!("{}: {e}", cmd.program.display()))
        })?;
        // Job objects exist only on Windows; elsewhere the worker's parent watchdog applies.
        let job = if cfg!(windows) {
            match KillOnCloseJob::assign(&child) {
                Ok(j) => Some(j),
                Err(e) => {
                    tracing::warn!(error = %e, "GPU worker not placed in a job object");
                    None
                }
            }
        } else {
            None
        };
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut wp = Self {
            child: Some(child),
            stdin,
            rx: mpsc::channel().1,
            readers: Vec::new(),
            stderr_tail: Arc::default(),
            _job: job,
        };
        let (tx, rx) = mpsc::channel();
        wp.rx = rx;
        if let Some(out) = stdout {
            let r = std::thread::Builder::new()
                .name("bench-gpu-stdout".into())
                .spawn(move || {
                    for line in BufReader::new(out).lines() {
                        let Ok(line) = line else { break };
                        let item = match protocol::decode(&line) {
                            Ok(Some(m)) => Line::Msg(m),
                            Ok(None) => continue,
                            Err(e) => Line::Bad(e),
                        };
                        if tx.send(item).is_err() {
                            return;
                        }
                    }
                    let _ = tx.send(Line::Eof);
                });
            match r {
                Ok(h) => wp.readers.push(h),
                Err(e) => {
                    return Err(AppError::internal("could not spawn GPU reader thread")
                        .with_detail(e.to_string()));
                }
            }
        }
        if let Some(mut err) = stderr {
            let tail = Arc::clone(&wp.stderr_tail);
            let r = std::thread::Builder::new()
                .name("bench-gpu-stderr".into())
                .spawn(move || {
                    let mut buf = [0u8; 1024];
                    while let Ok(n) = err.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        let mut t = tail.lock().unwrap_or_else(PoisonError::into_inner);
                        t.push_str(&String::from_utf8_lossy(&buf[..n]));
                        if t.len() > 2 * STDERR_TAIL {
                            let cut = t.len() - STDERR_TAIL;
                            let cut = (cut..t.len()).find(|i| t.is_char_boundary(*i)).unwrap_or(0);
                            t.drain(..cut);
                        }
                    }
                });
            match r {
                Ok(h) => wp.readers.push(h),
                Err(e) => {
                    return Err(AppError::internal("could not spawn GPU reader thread")
                        .with_detail(e.to_string()));
                }
            }
        }
        Ok(wp)
    }

    fn recv(&self, timeout: Duration) -> Result<Line, RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }

    /// Writes `cancel` and closes stdin (EOF is also a stop request).
    fn request_stop(&mut self) {
        if let Some(mut s) = self.stdin.take() {
            let _ = writeln!(s, "{}", protocol::CANCEL_LINE);
            let _ = s.flush();
        }
    }

    /// Waits up to `grace` for the exit, then kills. Always reaps the process and joins
    /// the readers. `Ok(status)` = exited by itself; `Err(())` = had to be killed.
    fn finish(&mut self, grace: Duration) -> Result<ExitStatus, ()> {
        self.request_stop();
        let Some(mut child) = self.child.take() else {
            return Err(());
        };
        let t0 = Instant::now();
        let outcome = loop {
            match child.try_wait() {
                Ok(Some(s)) => break Ok(s),
                Ok(None) if t0.elapsed() < grace => std::thread::sleep(Duration::from_millis(10)),
                Ok(None) | Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(());
                }
            }
        };
        for h in self.readers.drain(..) {
            let _ = h.join();
        }
        outcome
    }

    fn stderr_tail(&self) -> String {
        let t = self
            .stderr_tail
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        t.trim()
            .chars()
            .rev()
            .take(500)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    fn abnormal(&self, what: String) -> AppError {
        let tail = self.stderr_tail();
        let detail = if tail.is_empty() {
            what
        } else {
            format!("{what}; stderr: {tail}")
        };
        AppError::new(ErrorCode::Platform, ABNORMAL_EXIT).with_detail(detail)
    }
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        if self.child.is_some() || !self.readers.is_empty() {
            let _ = self.finish(Duration::ZERO);
        }
    }
}

fn exit_text(r: &Result<ExitStatus, ()>) -> String {
    match r {
        Ok(s) => match s.code() {
            Some(c) => format!("exit code {c} (0x{:08X})", c as u32),
            None => "terminated without exit code".into(),
        },
        Err(()) => "killed after the grace period".into(),
    }
}

// ---------------------------------------------------------------------------------------
// Capability probe
// ---------------------------------------------------------------------------------------

/// Which driver thresholds the stop temperature comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThresholdBasis {
    /// NVML slowdown / GPU max operating temperature (NVIDIA).
    Nvml,
    /// WDDM `TemperatureWarning` (any vendor whose driver reports it).
    Wddm,
}

/// Temperature policy for one GPU.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Thermal {
    pub thresholds: GpuThermalThresholds,
    /// Stop when the temperature reaches this value (see [`thermal_policy`]).
    pub stop_at_c: Option<f64>,
    pub basis: Option<ThresholdBasis>,
    /// A temperature could be read at probe time.
    pub temp_readable: bool,
    /// A threshold is known and a temperature could be read at probe time: the stop rule
    /// is in use (longer duration cap, stop when the sensor is lost).
    pub observable: bool,
}

impl Thermal {
    pub(crate) fn unobservable() -> Self {
        Self {
            thresholds: GpuThermalThresholds::default(),
            stop_at_c: None,
            basis: None,
            temp_readable: false,
            observable: false,
        }
    }

    pub(crate) fn max_duration_s(&self) -> u32 {
        if self.observable {
            MAX_S_OBSERVED
        } else {
            MAX_S_UNOBSERVED
        }
    }

    /// The stop temperature shown to the user: only when the stop rule is in use.
    pub(crate) fn stop_temperature_c(&self) -> Option<f64> {
        self.stop_at_c.filter(|_| self.observable)
    }

    /// Start warning when the stop rule is not in use (conservative duration cap).
    pub(crate) fn start_warning(&self, lang: Language) -> Option<&'static str> {
        match (self.observable, self.temp_readable) {
            (true, _) => None,
            (false, true) => Some(warn_gpu_no_threshold(lang)),
            (false, false) => Some(warn_gpu_temperature(lang)),
        }
    }
}

/// Stop temperature from the driver's own thresholds for this GPU:
/// 1. NVML (NVIDIA): the lower of *slowdown − [`TEMP_MARGIN_C`]* and *GPU max operating
///    temperature* (clocks may drop below base from there), whichever are exposed. The
///    shutdown threshold alone is not used (too close to a hardware trip to be a useful
///    stop point).
/// 2. Otherwise the WDDM `TemperatureWarning` the driver reports (any vendor), as is: it
///    is the driver's own warning level, no margin is subtracted.
/// 3. Otherwise none: no invented universal limit, the conservative duration cap applies.
///
/// The rule is in use (`observable`) only with a threshold and a current reading.
pub(crate) fn thermal_policy(th: GpuThermalThresholds, temp_now: Option<f64>) -> Thermal {
    let nvml = [th.slowdown_c.map(|s| s - TEMP_MARGIN_C), th.gpu_max_c]
        .into_iter()
        .flatten()
        .min_by(f64::total_cmp);
    let (stop_at_c, basis) = match (nvml, th.driver_warning_c) {
        (Some(s), _) => (Some(s), Some(ThresholdBasis::Nvml)),
        (None, Some(w)) => (Some(w), Some(ThresholdBasis::Wddm)),
        (None, None) => (None, None),
    };
    Thermal {
        thresholds: th,
        stop_at_c,
        basis,
        temp_readable: temp_now.is_some(),
        observable: stop_at_c.is_some() && temp_now.is_some(),
    }
}

/// The thresholds the stop temperature was derived from (UI copy in `lang`).
pub(crate) fn thresholds_text(t: &Thermal, lang: Language) -> String {
    let th = &t.thresholds;
    let names: &[(Option<f64>, &str, &str)] = match t.basis {
        Some(ThresholdBasis::Nvml) => &[
            (th.slowdown_c, "rallentamento", "slowdown"),
            (
                th.gpu_max_c,
                "temperatura operativa massima",
                "max operating temperature",
            ),
            (th.shutdown_c, "spegnimento", "shutdown"),
        ],
        Some(ThresholdBasis::Wddm) => &[
            (
                th.driver_warning_c,
                "soglia di avviso WDDM",
                "WDDM warning threshold",
            ),
            (th.driver_max_c, "massima WDDM", "WDDM maximum"),
        ],
        None => &[],
    };
    names
        .iter()
        .filter_map(|(v, it, en)| v.map(|v| format!("{} {v:.0} °C", lang.pick(it, en))))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GpuTarget {
    /// `GpuInfo::id` (DXGI LUID on Windows, I/O Kit registry id on macOS).
    pub id: String,
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
}

/// Everything a run on one adapter needs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GpuReady {
    pub target: GpuTarget,
    /// LUIDs the worker must never select: every other DXGI adapter (software, virtual and
    /// the other physical GPUs).
    pub exclude_luids: Vec<String>,
    /// What the probe matched (the run re-selects and reports the adapter it used).
    pub adapter: AdapterReport,
    pub matched_by: MatchKind,
    pub thermal: Thermal,
}

/// Why one adapter (or the whole machine) cannot run the benchmark. The probe result is
/// cached across language changes, so the user-facing texts are rendered on demand
/// ([`Unusable::render`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Unusable {
    /// The GPU list (DXGI on Windows, I/O Kit on macOS) could not be read.
    AdapterListFailed(AppError),
    /// Only software or virtual adapters.
    NoPhysicalGpu,
    /// No usable physical GPU (fallback when the default target has no reason).
    NoUsableGpu,
    /// The wgpu probe failed.
    ProbeFailed(AppError),
    /// wgpu has no (or no unambiguous) adapter of the worker's backends ([`COMPUTE_API`])
    /// for the GPU named `gpu`.
    NoComputeAdapter { gpu: String, why: NoMatch },
}

/// Where the GPU list comes from, as named in user-facing texts.
#[cfg(not(target_os = "macos"))]
const ADAPTER_LIST_API: &str = "DXGI";
#[cfg(target_os = "macos")]
const ADAPTER_LIST_API: &str = "I/O Kit";

/// [`Unusable`] in one language.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TargetUnavailable {
    pub reason: Unavailability,
    /// Plain-language explanation.
    pub note: String,
}

impl Unusable {
    pub(crate) fn render(&self, lang: Language) -> TargetUnavailable {
        let (reason, note) = match self {
            Self::AdapterListFailed(e) => (
                Unavailability::Failed(e.clone()),
                lang.pick(
                    format!("Elenco delle GPU ({ADAPTER_LIST_API}) non disponibile."),
                    format!("GPU list ({ADAPTER_LIST_API}) unavailable."),
                ),
            ),
            Self::NoPhysicalGpu => (
                Unavailability::NotSupported,
                lang.pick(
                    "Nessuna GPU fisica rilevata (solo adattatori software o virtuali).",
                    "No physical GPU detected (only software or virtual adapters).",
                )
                .to_owned(),
            ),
            Self::NoUsableGpu => (
                Unavailability::NotSupported,
                lang.pick(
                    "Nessuna GPU fisica utilizzabile.",
                    "No usable physical GPU.",
                )
                .to_owned(),
            ),
            Self::ProbeFailed(e) => (
                Unavailability::Failed(e.clone()),
                lang.pick(
                    format!("Verifica del supporto wgpu ({COMPUTE_API}) non riuscita."),
                    format!("The wgpu ({COMPUTE_API}) support check failed."),
                ),
            ),
            Self::NoComputeAdapter { gpu, why } => (
                Unavailability::ProviderMissing(lang.pick(
                    format!("adattatore wgpu {COMPUTE_API} per {gpu}"),
                    format!("wgpu {COMPUTE_API} adapter for {gpu}"),
                )),
                format!("GPU {gpu}: {}.", why.text(lang)),
            ),
        };
        TargetUnavailable { reason, note }
    }
}

/// One physical adapter and its probe outcome.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TargetEnv {
    pub target: GpuTarget,
    /// The target used when the request names none (the primary physical GPU).
    pub is_default: bool,
    pub thermal: Thermal,
    pub status: Result<Box<GpuReady>, Unusable>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GpuEnv {
    /// Every physical DXGI adapter, primary first (non-empty, exactly one default).
    Targets(Vec<TargetEnv>),
    /// No adapter list, or no physical adapter at all (explained in the capability
    /// description).
    Unavailable(Unusable),
}

impl GpuEnv {
    pub(crate) fn targets(&self) -> &[TargetEnv] {
        match self {
            Self::Targets(t) => t,
            Self::Unavailable(_) => &[],
        }
    }

    pub(crate) fn default_target(&self) -> Option<&TargetEnv> {
        self.targets().iter().find(|t| t.is_default)
    }

    pub(crate) fn find(&self, id: &str) -> Option<&TargetEnv> {
        self.targets()
            .iter()
            .find(|t| t.target.id.eq_ignore_ascii_case(id))
    }

    /// Kind-level availability: available if at least one target is, else the default
    /// (primary) target's reason.
    pub(crate) fn availability(&self) -> Result<(), Unusable> {
        match self {
            Self::Unavailable(u) => Err(u.clone()),
            Self::Targets(t) if t.iter().any(|t| t.status.is_ok()) => Ok(()),
            Self::Targets(_) => Err(self
                .default_target()
                .and_then(|t| t.status.clone().err())
                .unwrap_or(Unusable::NoUsableGpu)),
        }
    }
}

#[cfg(test)]
impl GpuEnv {
    pub(crate) fn ready(&self) -> Option<&GpuReady> {
        self.default_target()
            .and_then(|t| t.status.as_ref().ok())
            .map(|b| &**b)
    }
}

#[cfg(test)]
pub(crate) const TEST_GPU_ID: &str = "luid:00000000-00000001";
#[cfg(test)]
pub(crate) const TEST_IGPU_ID: &str = "luid:00000000-00000002";

/// A ready single-GPU environment for tests (no real GPU): `observable` selects the
/// thermal policy (NVML-like slowdown 94 °C, or nothing readable).
#[cfg(test)]
pub(crate) fn test_env(observable: bool) -> GpuEnv {
    let thermal = if observable {
        thermal_policy(
            GpuThermalThresholds {
                slowdown_c: Some(94.0),
                ..GpuThermalThresholds::default()
            },
            Some(40.0),
        )
    } else {
        Thermal::unobservable()
    };
    GpuEnv::Targets(vec![test_target(
        TEST_GPU_ID,
        "Test GPU",
        0x10DE,
        true,
        thermal,
        true,
    )])
}

/// Two targets for tests: an observable default "Test GPU" and a "Test iGPU" whose
/// temperature is readable without thresholds; `igpu_ready` = whether wgpu matched it.
#[cfg(test)]
pub(crate) fn test_env_two(igpu_ready: bool) -> GpuEnv {
    let mut targets = test_env(true).targets().to_vec();
    targets.push(test_target(
        TEST_IGPU_ID,
        "Test iGPU",
        0x1002,
        false,
        thermal_policy(GpuThermalThresholds::default(), Some(45.0)),
        igpu_ready,
    ));
    GpuEnv::Targets(targets)
}

#[cfg(test)]
fn test_target(
    id: &str,
    name: &str,
    vendor_id: u32,
    is_default: bool,
    thermal: Thermal,
    ready: bool,
) -> TargetEnv {
    let target = GpuTarget {
        id: id.into(),
        name: name.into(),
        vendor_id,
        device_id: 0x1,
    };
    let status = if ready {
        Ok(Box::new(GpuReady {
            target: target.clone(),
            exclude_luids: Vec::new(),
            adapter: AdapterReport {
                name: name.into(),
                vendor_id,
                device_id: 0x1,
                backend: "dx12".into(),
                device_type: "discrete".into(),
                luid: Some(id.into()),
                driver: String::new(),
                driver_info: String::new(),
            },
            matched_by: MatchKind::Luid,
            thermal: thermal.clone(),
        }))
    } else {
        Err(Unusable::NoComputeAdapter {
            gpu: name.into(),
            why: NoMatch::NoAdapter {
                vendor_id,
                device_id: 0x1,
                luid: Some(id.into()),
            },
        })
    };
    TargetEnv {
        target,
        is_default,
        thermal,
        status,
    }
}

/// Pure decision: every physical DXGI adapter (not software, not virtual) in DXGI order,
/// the first one (the UI's `primaryGpu` rule) as default, each matched against the wgpu
/// adapters of one probe. The probe is skipped when there is no physical adapter.
pub(crate) fn decide(
    dxgi: Result<Vec<GpuInfo>, AppError>,
    probe: impl FnOnce() -> Result<Vec<AdapterReport>, AppError>,
    mut thermal: impl FnMut(&GpuInfo) -> Thermal,
) -> GpuEnv {
    let gpus = match dxgi {
        Ok(g) => g,
        Err(e) => return GpuEnv::Unavailable(Unusable::AdapterListFailed(e)),
    };
    let physical: Vec<&GpuInfo> = gpus
        .iter()
        .filter(|g| !g.is_software && !g.is_virtual)
        .collect();
    if physical.is_empty() {
        return GpuEnv::Unavailable(Unusable::NoPhysicalGpu);
    }
    let adapters = probe();
    let targets = physical
        .iter()
        .enumerate()
        .map(|(i, g)| {
            // 0 = no PCI device id (Apple silicon). wgpu reports 0 for those adapters too,
            // so the PCI-id fallback in `select` still compares like with like; the LUID /
            // registry id match comes first anyway.
            let device_id = g.device_id.value().copied().unwrap_or(0);
            let target = GpuTarget {
                id: g.id.clone(),
                name: g.name.clone(),
                vendor_id: g.vendor_id,
                device_id,
            };
            let th = thermal(g);
            let exclude_luids: Vec<String> = gpus
                .iter()
                .filter(|o| o.id != g.id)
                .map(|o| o.id.clone())
                .collect();
            let status = match &adapters {
                Err(e) => Err(Unusable::ProbeFailed(e.clone())),
                Ok(adapters) => {
                    let want = Target {
                        luid: Some(g.id.clone()),
                        vendor_id: g.vendor_id,
                        device_id,
                    };
                    match select::select(adapters, &want, &exclude_luids) {
                        Ok((k, matched_by)) => Ok(Box::new(GpuReady {
                            target: target.clone(),
                            exclude_luids,
                            adapter: adapters[k].clone(),
                            matched_by,
                            thermal: th.clone(),
                        })),
                        Err(why) => Err(Unusable::NoComputeAdapter {
                            gpu: g.name.clone(),
                            why,
                        }),
                    }
                }
            };
            TargetEnv {
                target,
                is_default: i == 0,
                thermal: th,
                status,
            }
        })
        .collect();
    GpuEnv::Targets(targets)
}

fn probe_adapters(cmd: &WorkerCommand) -> Result<Vec<AdapterReport>, AppError> {
    let mut wp = WorkerProcess::spawn(cmd, &WorkerArgs::probe().to_args())?;
    let t0 = Instant::now();
    let mut found = None;
    let mut error = None;
    while t0.elapsed() < PROBE_TIMEOUT {
        match wp.recv(Duration::from_millis(50)) {
            Ok(Line::Msg(Msg::Adapters { adapters })) => {
                found = Some(adapters);
                break;
            }
            Ok(Line::Msg(Msg::Error {
                code,
                message,
                detail,
            })) => {
                let mut e = AppError::new(code, message);
                e.detail = detail;
                error = Some(e);
                break;
            }
            Ok(Line::Msg(_)) | Err(RecvTimeoutError::Timeout) => {}
            Ok(Line::Bad(e)) => {
                error = Some(AppError::internal("GPU worker protocol error").with_detail(e));
                break;
            }
            Ok(Line::Eof) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = wp.finish(CANCEL_GRACE);
    match (found, error) {
        (Some(a), _) => Ok(a),
        (None, Some(e)) => Err(e),
        (None, None) => Err(wp.abnormal(format!("adapter probe: {}", exit_text(&status)))),
    }
}

/// Probes the machine: one worker spawn in `--probe` mode lists every wgpu adapter, each
/// physical DXGI adapter is matched against that list, and the driver thresholds and a
/// temperature reading are taken per adapter (NVML or WDDM).
pub(crate) fn probe_env(cmd: Result<WorkerCommand, AppError>) -> GpuEnv {
    let t0 = Instant::now();
    let env = decide(
        platform::gpu_bench::gpu_adapters(),
        || cmd.and_then(|c| probe_adapters(&c)),
        |g| match platform::gpu_bench::GpuSensor::open(&g.id) {
            Some(s) => thermal_policy(s.thresholds(), s.read().temperature_c),
            None => Thermal::unobservable(),
        },
    );
    tracing::info!(
        elapsed_ms = t0.elapsed().as_millis() as u64,
        ?env,
        "GPU benchmark probe"
    );
    env
}

// ---------------------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------------------

/// Everything a GPU run needs, resolved at `start`.
#[derive(Debug, Clone)]
pub(crate) struct GpuPlan {
    pub ready: GpuReady,
    pub command: WorkerCommand,
    pub warmup: Duration,
    pub simulate: Option<Simulate>,
}

fn adapter_text(a: &AdapterReport, how: MatchKind, lang: Language) -> String {
    if a.backend == "metal" {
        return metal_adapter_text(a, how, lang);
    }
    let how = match how {
        MatchKind::Luid => lang.pick("selezionato per LUID", "selected by LUID"),
        MatchKind::PciIds => lang.pick(
            "selezionato per ID PCI (LUID non verificabile su questo backend)",
            "selected by PCI ID (LUID not verifiable on this backend)",
        ),
    };
    let adapter = format!(
        "{} ({}, PCI {:04X}:{:04X}{})",
        a.name,
        a.backend.to_uppercase(),
        a.vendor_id,
        a.device_id,
        a.luid
            .as_deref()
            .map(|l| format!(", {l}"))
            .unwrap_or_default()
    );
    lang.pick(
        format!("Adattatore GPU utilizzato: {adapter}, {how}."),
        format!("GPU adapter used: {adapter}, {how}."),
    )
}

/// Metal adapters have no PCI ids and are matched by their I/O Kit registry id.
fn metal_adapter_text(a: &AdapterReport, how: MatchKind, lang: Language) -> String {
    let id = a
        .luid
        .as_deref()
        .map(|l| format!(", {l}"))
        .unwrap_or_default();
    let adapter = format!("{} (Metal{id})", a.name);
    let how = match how {
        MatchKind::Luid => lang.pick(
            "selezionato per ID del registro I/O Kit",
            "selected by I/O Kit registry ID",
        ),
        MatchKind::PciIds => lang.pick("selezionato per ID PCI", "selected by PCI ID"),
    };
    lang.pick(
        format!("Adattatore GPU utilizzato: {adapter}, {how}."),
        format!("GPU adapter used: {adapter}, {how}."),
    )
}

/// The worker reports phases as stable keys ([`protocol::phase`]); the parent words them.
fn phase_text(key: &str, lang: Language) -> &'static str {
    match key {
        protocol::phase::SELECT => {
            lang.pick("Selezione adattatore GPU", "Selecting the GPU adapter")
        }
        protocol::phase::COMPILE => lang.pick("Compilazione shader", "Compiling the shader"),
        protocol::phase::WARMUP => lang.pick("Riscaldamento", "Warming up"),
        protocol::phase::MEASURE => lang.pick("Misura", "Measuring"),
        protocol::phase::VERIFY => lang.pick("Verifica del risultato", "Verifying the result"),
        _ => lang.pick("Benchmark GPU", "GPU benchmark"),
    }
}

pub(crate) fn run(ctx: &RunCtx, plan: &Plan) -> Result<Outcome, RunError> {
    let Some(gp) = plan.gpu.as_ref() else {
        return Err(RunError::Failed(AppError::internal("GPU plan missing")));
    };
    let duration = plan
        .duration
        .unwrap_or(Duration::from_secs(u64::from(DEFAULT_S)));
    let args = WorkerArgs {
        probe: false,
        duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        warmup_ms: u64::try_from(gp.warmup.as_millis()).unwrap_or(u64::MAX),
        target: Target {
            luid: Some(gp.ready.target.id.clone()),
            vendor_id: gp.ready.target.vendor_id,
            device_id: gp.ready.target.device_id,
        },
        exclude_luids: gp.ready.exclude_luids.clone(),
        simulate: gp.simulate,
    };
    let lang = ctx.lang;
    ctx.set_phase(
        lang.pick("Avvio del processo GPU", "Starting the GPU process"),
        0.0,
    );
    let mut wp = WorkerProcess::spawn(&gp.command, &args.to_args())?;
    let stall = ctx.hooks.gpu_stall.unwrap_or(STALL_TIMEOUT);
    let mut last_msg = Instant::now();
    let mut selected: Option<(AdapterReport, MatchKind)> = None;
    let mut measuring_at: Option<u64> = None;
    let mut result: Option<WorkerResult> = None;
    let mut failure: Option<AppError> = None;
    loop {
        if ctx.is_cancelled() {
            let t = Instant::now();
            let r = wp.finish(CANCEL_GRACE);
            tracing::info!(
                stopped_in_ms = t.elapsed().as_millis() as u64,
                how = %exit_text(&r),
                "GPU worker stopped"
            );
            return Err(RunError::Cancelled);
        }
        match wp.recv(Duration::from_millis(25)) {
            Ok(Line::Msg(m)) => {
                last_msg = Instant::now();
                match m {
                    Msg::Hello { pid } => {
                        if let Some(log) = &ctx.hooks.gpu_pids {
                            log.lock().unwrap_or_else(PoisonError::into_inner).push(pid);
                        }
                    }
                    Msg::Selected {
                        adapter,
                        matched_by,
                    } => {
                        tracing::info!(adapter = %adapter.name, ?matched_by, "GPU adapter selected");
                        selected = Some((adapter, matched_by));
                    }
                    Msg::Measuring => measuring_at = Some(ctx.elapsed_ms()),
                    Msg::Progress {
                        phase,
                        fraction,
                        flops_per_s,
                    } => {
                        ctx.set_phase(phase_text(&phase, lang), fraction);
                        if let Some(f) = flops_per_s {
                            ctx.set_interim(vec![throughput_metric(f, lang)]);
                        }
                    }
                    Msg::Done { result: r } => {
                        result = Some(r);
                        break;
                    }
                    Msg::Error {
                        code,
                        message,
                        detail,
                    } => {
                        let mut e = AppError::new(code, message);
                        e.detail = detail;
                        failure = Some(e);
                        break;
                    }
                    Msg::Adapters { .. } => {}
                }
            }
            Ok(Line::Bad(e)) => {
                failure = Some(wp.abnormal(format!("protocol error: {e}")));
                break;
            }
            Ok(Line::Eof) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                if last_msg.elapsed() > stall {
                    let r = wp.finish(Duration::ZERO);
                    return Err(RunError::Failed(wp.abnormal(format!(
                        "no message for {} s, process killed ({})",
                        stall.as_secs(),
                        exit_text(&r)
                    ))));
                }
            }
        }
    }
    let status = wp.finish(EXIT_GRACE);
    if ctx.is_cancelled() {
        return Err(RunError::Cancelled);
    }
    if let Some(e) = failure {
        return Err(RunError::Failed(e));
    }
    let Some(result) = result else {
        return Err(RunError::Failed(wp.abnormal(format!(
            "process ended without a result ({})",
            exit_text(&status)
        ))));
    };
    let Some((adapter, how)) = selected else {
        return Err(RunError::Failed(AppError::internal(
            "GPU worker reported a result without an adapter",
        )));
    };
    Ok(build_outcome(
        &result,
        &adapter,
        how,
        measuring_at.unwrap_or(0),
        lang,
    ))
}

fn throughput_metric(flops_per_s: f64, lang: Language) -> models::benchmark_run::ResultMetric {
    metric(
        "throughput",
        lang.pick("Throughput FP32 (FLOP/s)", "FP32 throughput (FLOP/s)"),
        flops_per_s,
        Unit::FlopsPerSecond,
        true,
    )
}

/// Pure: metrics, series, warnings and notes (in `lang`) from the worker result.
/// `offset_ms` is the run-relative time at which the timed window started.
pub(crate) fn build_outcome(
    r: &WorkerResult,
    adapter: &AdapterReport,
    how: MatchKind,
    offset_ms: u64,
    lang: Language,
) -> Outcome {
    let secs = r.timed_ms as f64 / 1000.0;
    let mut metrics = vec![throughput_metric(
        if secs > 0.0 {
            r.total_flops / secs
        } else {
            f64::NAN
        },
        lang,
    )];
    let values: Vec<f64> = r.points.iter().map(|p| p.flops_per_s).collect();
    if let Some(change) = stats::first_last_decile_change_percent(&values) {
        metrics.push(metric(
            "throughput_drop_percent",
            lang.pick("Calo tra inizio e fine", "Drop between start and end"),
            -change,
            Unit::Percent,
            false,
        ));
    }
    if let Some(cv) = stats::cv_percent(&values) {
        metrics.push(metric(
            "throughput_cv",
            lang.pick("Coefficiente di variazione", "Coefficient of variation"),
            cv,
            Unit::Percent,
            false,
        ));
    }
    let series = vec![BenchmarkSeries {
        id: "throughput".into(),
        label: lang
            .pick("Throughput FP32 per secondo", "FP32 throughput per second")
            .into(),
        unit: Unit::FlopsPerSecond,
        points: r
            .points
            .iter()
            .map(|p| SeriesPoint {
                elapsed_ms: offset_ms + p.elapsed_ms,
                value: p.flops_per_s,
            })
            .collect(),
    }];
    let notes = vec![adapter_text(adapter, how, lang)];
    let mut warnings = Vec::new();
    if let Some(drop) = metrics
        .iter()
        .find(|m| m.id == "throughput_drop_percent")
        .map(|m| m.value)
        && drop >= 10.0
    {
        warnings.push(lang.pick(
            format!(
                "Possibile riduzione sostenuta delle prestazioni GPU durante il test ({drop:.1}% \
                 tra inizio e fine), causa non determinabile."
            ),
            format!(
                "Possible sustained GPU performance reduction during the test ({drop:.1}% \
                 between start and end), cause undetermined."
            ),
        ));
    }
    debug_assert_eq!(r.flops_per_invocation, kernel::flops_per_invocation());
    Outcome {
        metrics,
        series,
        warnings,
        notes,
    }
}

// ---------------------------------------------------------------------------------------
// Safety during the run (called by the supervisor every 250 ms)
// ---------------------------------------------------------------------------------------

/// Temperature watch and GPU telemetry series. Readings come from the live monitor
/// (`latest()`) when it has a fresh value for this GPU, else directly from the sensor
/// (NVML on NVIDIA, WDDM performance data otherwise; power only through NVML).
pub(crate) struct GpuWatch {
    id: String,
    sensor: Option<platform::gpu_bench::GpuSensor>,
    thermal: Thermal,
    /// Language of the stop reasons and series labels.
    lang: Language,
    last_check: Option<Instant>,
    last_seq: Option<u64>,
    missing: u32,
    pub temp: Vec<SeriesPoint>,
    pub clock: Vec<SeriesPoint>,
    pub power: Vec<SeriesPoint>,
}

/// One reading: temperature, clock, power.
pub(crate) type Reading = (Option<f64>, Option<f64>, Option<f64>);

impl GpuWatch {
    pub(crate) fn new(ready: &GpuReady, open_sensor: bool, lang: Language) -> Self {
        Self {
            id: ready.target.id.clone(),
            sensor: open_sensor
                .then(|| platform::gpu_bench::GpuSensor::open(&ready.target.id))
                .flatten(),
            thermal: ready.thermal.clone(),
            lang,
            last_check: None,
            last_seq: None,
            missing: 0,
            temp: Vec::new(),
            clock: Vec::new(),
            power: Vec::new(),
        }
    }

    fn read(&mut self, latest: Option<models::monitor::LiveSample>) -> Reading {
        if let Some(s) = latest
            && self.last_seq != Some(s.seq)
            && let Some(g) = s.gpus.iter().find(|g| g.id == self.id)
            && g.temperature_c.is_available()
        {
            self.last_seq = Some(s.seq);
            return (
                g.temperature_c.value().copied(),
                g.core_clock_mhz.value().copied(),
                g.power_w.value().copied(),
            );
        }
        match &self.sensor {
            Some(s) => {
                let r = s.read();
                (r.temperature_c, r.core_clock_mhz, r.power_w)
            }
            None => (None, None, None),
        }
    }

    /// Every second: records the series and returns a stop reason, if any.
    pub(crate) fn tick(
        &mut self,
        elapsed_ms: u64,
        latest: Option<models::monitor::LiveSample>,
    ) -> Option<String> {
        if self
            .last_check
            .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
        {
            return None;
        }
        self.last_check = Some(Instant::now());
        let reading = self.read(latest);
        self.check(elapsed_ms, reading)
    }

    /// Pure part of `tick` (unit-tested).
    pub(crate) fn check(&mut self, elapsed_ms: u64, (t, c, p): Reading) -> Option<String> {
        for (series, v) in [
            (&mut self.temp, t),
            (&mut self.clock, c),
            (&mut self.power, p),
        ] {
            if let Some(value) = v {
                series.push(SeriesPoint { elapsed_ms, value });
            }
        }
        let lang = self.lang;
        match (t, self.thermal.stop_at_c) {
            (Some(temp), Some(stop)) if temp >= stop => {
                let th = thresholds_text(&self.thermal, lang);
                Some(lang.pick(
                    format!(
                        "Temperatura GPU {temp:.0} °C: raggiunto il limite di sicurezza di \
                         {stop:.0} °C derivato dalle soglie del driver ({th}): test interrotto."
                    ),
                    format!(
                        "GPU temperature {temp:.0} °C: reached the {stop:.0} °C safety limit \
                         derived from the driver's thresholds ({th}): test stopped."
                    ),
                ))
            }
            (Some(_), _) => {
                self.missing = 0;
                None
            }
            (None, _) if self.thermal.observable => {
                self.missing += 1;
                (self.missing >= TEMP_LOST_CHECKS).then(|| {
                    lang.pick(
                        "Temperatura GPU non più leggibile durante il test: interrotto per \
                         sicurezza.",
                        "GPU temperature no longer readable during the test: stopped for \
                         safety.",
                    )
                    .to_owned()
                })
            }
            (None, _) => None,
        }
    }

    pub(crate) fn into_series(self) -> Vec<BenchmarkSeries> {
        let lang = self.lang;
        let mut out = Vec::new();
        for (id, label, unit, points) in [
            (
                "gpu_temp_c",
                lang.pick("Temperatura GPU", "GPU temperature"),
                Unit::Celsius,
                self.temp,
            ),
            (
                "gpu_clock_mhz",
                lang.pick("Frequenza GPU", "GPU clock"),
                Unit::Megahertz,
                self.clock,
            ),
            (
                "gpu_power_w",
                lang.pick("Potenza GPU", "GPU power"),
                Unit::Watts,
                self.power,
            ),
        ] {
            if !points.is_empty() {
                out.push(BenchmarkSeries {
                    id: id.into(),
                    label: label.into(),
                    unit,
                    points,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_worker::protocol::Point;

    const IT: Language = Language::Italian;
    const EN: Language = Language::English;
    use models::metric::{Metric, Source};

    fn info(id: &str, name: &str, vendor: u32, device: u32, sw: bool, virt: bool) -> GpuInfo {
        GpuInfo {
            id: id.into(),
            name: name.into(),
            vendor: models::hardware::GpuVendor::from_pci_id(vendor),
            vendor_id: vendor,
            device_id: Metric::available(device, Unit::None, Source::Dxgi),
            dedicated_vram_bytes: Metric::available(0.0, Unit::Bytes, Source::Dxgi),
            shared_memory_bytes: Metric::available(0.0, Unit::Bytes, Source::Dxgi),
            is_software: sw,
            is_virtual: virt,
            driver_version: Metric::unavailable(Unavailability::NotSupported),
            telemetry_sources: Vec::new(),
        }
    }

    fn report(
        name: &str,
        backend: &str,
        ty: &str,
        v: u32,
        d: u32,
        luid: Option<&str>,
    ) -> AdapterReport {
        AdapterReport {
            name: name.into(),
            vendor_id: v,
            device_id: d,
            backend: backend.into(),
            device_type: ty.into(),
            luid: luid.map(Into::into),
            driver: String::new(),
            driver_info: String::new(),
        }
    }

    const RTX: &str = "luid:00000000-0000D1B3";
    const PARSEC: &str = "luid:00000000-0000E000";

    fn machine_dxgi() -> Vec<GpuInfo> {
        vec![
            info(
                PARSEC,
                "Parsec Virtual Display Adapter",
                0x10DE,
                0x2783,
                false,
                true,
            ),
            info(
                RTX,
                "NVIDIA GeForce RTX 4070 SUPER",
                0x10DE,
                0x2783,
                false,
                false,
            ),
            info(
                "luid:00000000-0000A000",
                "AMD Radeon(TM) Graphics",
                0x1002,
                0x164E,
                false,
                false,
            ),
            info(
                "luid:00000000-00000100",
                "Microsoft Basic Render Driver",
                0x1414,
                0x8C,
                true,
                false,
            ),
        ]
    }

    fn machine_wgpu() -> Vec<AdapterReport> {
        vec![
            report("Parsec", "dx12", "discrete", 0x10DE, 0x2783, Some(PARSEC)),
            report(
                "NVIDIA GeForce RTX 4070 SUPER",
                "dx12",
                "discrete",
                0x10DE,
                0x2783,
                Some(RTX),
            ),
            report(
                "Microsoft Basic Render Driver",
                "dx12",
                "cpu",
                0x1414,
                0x8C,
                Some("luid:00000000-00000100"),
            ),
        ]
    }

    const AMD: &str = "luid:00000000-0000A000";

    fn machine_wgpu_with_igpu() -> Vec<AdapterReport> {
        let mut v = machine_wgpu();
        v.push(report(
            "AMD Radeon(TM) Graphics",
            "dx12",
            "integrated",
            0x1002,
            0x164E,
            Some(AMD),
        ));
        v
    }

    #[test]
    fn capability_probe_is_honest() {
        // Every physical GPU (virtual and software skipped), primary first, matched by LUID.
        let env = decide(
            Ok(machine_dxgi()),
            || Ok(machine_wgpu_with_igpu()),
            |_| Thermal::unobservable(),
        );
        let ids: Vec<&str> = env.targets().iter().map(|t| t.target.id.as_str()).collect();
        assert_eq!(ids, [RTX, AMD]);
        let defaults: Vec<bool> = env.targets().iter().map(|t| t.is_default).collect();
        assert_eq!(defaults, [true, false]);
        let r = env.ready().expect("ready");
        assert_eq!(r.target.id, RTX);
        assert_eq!(r.adapter.luid.as_deref(), Some(RTX));
        assert_eq!(r.matched_by, MatchKind::Luid);
        // Software, virtual and the other physical adapter are excluded; never itself.
        assert!(r.exclude_luids.contains(&PARSEC.to_owned()));
        assert!(r.exclude_luids.contains(&AMD.to_owned()));
        assert!(!r.exclude_luids.contains(&RTX.to_owned()));
        assert_eq!(r.thermal.max_duration_s(), MAX_S_UNOBSERVED);
        let igpu = env.find(AMD).expect("igpu");
        let ir = igpu.status.as_ref().expect("igpu ready");
        assert_eq!(ir.adapter.name, "AMD Radeon(TM) Graphics");
        assert!(ir.exclude_luids.contains(&RTX.to_owned()));
        assert_eq!(env.availability(), Ok(()));

        // wgpu does not see the iGPU: that target alone is unavailable, the kind is not.
        let env = decide(
            Ok(machine_dxgi()),
            || Ok(machine_wgpu()),
            |_| Thermal::unobservable(),
        );
        let igpu = env.find(AMD).expect("igpu");
        match igpu.status.as_ref().map_err(|u| u.render(IT)) {
            Err(TargetUnavailable {
                reason: Unavailability::ProviderMissing(d),
                note,
            }) => {
                assert!(d.contains("AMD Radeon"), "{d}");
                assert!(note.contains("nessun adattatore"), "{note}");
            }
            other => panic!("{other:?}"),
        }
        match igpu.status.as_ref().map_err(|u| u.render(EN)) {
            Err(TargetUnavailable {
                reason: Unavailability::ProviderMissing(d),
                note,
            }) => {
                let api = if cfg!(target_os = "macos") {
                    "Metal"
                } else {
                    "DX12/Vulkan"
                };
                assert_eq!(d, format!("wgpu {api} adapter for AMD Radeon(TM) Graphics"));
                assert_eq!(
                    note,
                    format!(
                        "GPU AMD Radeon(TM) Graphics: no physical {api} adapter matches the \
                         requested GPU (PCI 1002:164E, luid:00000000-0000A000)."
                    )
                );
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(env.availability(), Ok(()));

        // Only software/virtual adapters.
        let only_bad: Vec<GpuInfo> = machine_dxgi()
            .into_iter()
            .filter(|g| g.is_software || g.is_virtual)
            .collect();
        let env = decide(
            Ok(only_bad),
            || panic!("must not probe"),
            |_| Thermal::unobservable(),
        );
        assert!(matches!(env, GpuEnv::Unavailable(Unusable::NoPhysicalGpu)));
        assert_eq!(
            env.availability().map_err(|u| u.render(EN).note),
            Err("No physical GPU detected (only software or virtual adapters).".to_owned())
        );
        assert!(env.targets().is_empty());

        // wgpu sees only WARP and the virtual adapter: every target unavailable, the kind
        // takes the primary's reason.
        let wg: Vec<AdapterReport> = machine_wgpu()
            .into_iter()
            .filter(|a| a.luid.as_deref() != Some(RTX))
            .collect();
        let env = decide(Ok(machine_dxgi()), || Ok(wg), |_| Thermal::unobservable());
        assert!(env.targets().iter().all(|t| t.status.is_err()));
        match env.availability().map_err(|u| u.render(IT)) {
            Err(TargetUnavailable {
                reason: Unavailability::ProviderMissing(d),
                note,
            }) => {
                assert!(d.contains("RTX 4070 SUPER"), "{d}");
                assert!(note.contains("nessun adattatore"), "{note}");
            }
            other => panic!("{other:?}"),
        }

        // Probe or DXGI failures are reported as failures, not as "not supported".
        let env = decide(
            Ok(machine_dxgi()),
            || Err(AppError::internal("boom")),
            |_| Thermal::unobservable(),
        );
        assert!(matches!(
            env.availability().map_err(|u| u.render(IT)),
            Err(TargetUnavailable {
                reason: Unavailability::Failed(_),
                ..
            })
        ));
        let env = decide(
            Err(AppError::internal("dxgi")),
            || Ok(vec![]),
            |_| Thermal::unobservable(),
        );
        assert!(matches!(
            env,
            GpuEnv::Unavailable(Unusable::AdapterListFailed(_))
        ));
    }

    #[test]
    fn probe_takes_thermal_per_target() {
        let env = decide(
            Ok(machine_dxgi()),
            || Ok(machine_wgpu_with_igpu()),
            |g| {
                if g.id == RTX {
                    thermal_policy(
                        GpuThermalThresholds {
                            slowdown_c: Some(96.0),
                            gpu_max_c: Some(90.0),
                            driver_warning_c: Some(84.0),
                            ..GpuThermalThresholds::default()
                        },
                        Some(40.0),
                    )
                } else {
                    thermal_policy(GpuThermalThresholds::default(), Some(49.0))
                }
            },
        );
        let rtx = env.find(RTX).expect("rtx");
        assert_eq!(rtx.thermal.stop_temperature_c(), Some(90.0));
        assert_eq!(rtx.thermal.start_warning(IT), None);
        let igpu = env.find(&AMD.to_lowercase()).expect("igpu");
        assert_eq!(igpu.thermal.stop_temperature_c(), None);
        assert_eq!(
            igpu.thermal.start_warning(IT),
            Some(warn_gpu_no_threshold(IT))
        );
        assert_eq!(igpu.thermal.max_duration_s(), MAX_S_UNOBSERVED);
    }

    #[test]
    fn thermal_policy_uses_driver_thresholds_only() {
        // RTX 4070 SUPER on the test machine: NVML slowdown 96, max operating 90, shutdown
        // 101; WDDM warning 84, max 90. NVML present → the NVML rule, WDDM ignored.
        let th = GpuThermalThresholds {
            slowdown_c: Some(96.0),
            shutdown_c: Some(101.0),
            gpu_max_c: Some(90.0),
            driver_warning_c: Some(84.0),
            driver_max_c: Some(90.0),
        };
        let t = thermal_policy(th, Some(45.0));
        assert_eq!(t.stop_at_c, Some(90.0));
        assert_eq!(t.basis, Some(ThresholdBasis::Nvml));
        assert!(t.observable);
        assert_eq!(t.max_duration_s(), MAX_S_OBSERVED);
        assert_eq!(t.stop_temperature_c(), Some(90.0));
        assert_eq!(t.start_warning(IT), None);
        assert!(thresholds_text(&t, IT).contains("rallentamento 96 °C"));
        assert!(!thresholds_text(&t, IT).contains("WDDM"));
        // Slowdown − 5 when it is the lower one.
        let t = thermal_policy(
            GpuThermalThresholds {
                gpu_max_c: Some(93.0),
                ..th
            },
            Some(45.0),
        );
        assert_eq!(t.stop_at_c, Some(91.0));
        // Only one of them exposed.
        let t = thermal_policy(
            GpuThermalThresholds {
                slowdown_c: None,
                ..th
            },
            Some(45.0),
        );
        assert_eq!(t.stop_at_c, Some(90.0));
        let t = thermal_policy(
            GpuThermalThresholds {
                gpu_max_c: None,
                ..th
            },
            Some(45.0),
        );
        assert_eq!(t.stop_at_c, Some(91.0));

        // WDDM only (e.g. AMD/Intel driver reporting TemperatureWarning): stop at it as is.
        let wddm = GpuThermalThresholds {
            driver_warning_c: Some(95.0),
            driver_max_c: Some(105.0),
            ..GpuThermalThresholds::default()
        };
        let t = thermal_policy(wddm, Some(50.0));
        assert_eq!(t.stop_at_c, Some(95.0));
        assert_eq!(t.basis, Some(ThresholdBasis::Wddm));
        assert!(t.observable);
        assert_eq!(t.max_duration_s(), MAX_S_OBSERVED);
        assert!(
            thresholds_text(&t, IT).contains("soglia di avviso WDDM 95 °C"),
            "{}",
            thresholds_text(&t, IT)
        );
        // WDDM max alone is not a stop point either.
        let t = thermal_policy(
            GpuThermalThresholds {
                driver_warning_c: None,
                ..wddm
            },
            Some(50.0),
        );
        assert_eq!((t.stop_at_c, t.observable), (None, false));
        // Shutdown alone is not a stop point.
        let only_shutdown = GpuThermalThresholds {
            shutdown_c: Some(101.0),
            ..GpuThermalThresholds::default()
        };
        assert!(!thermal_policy(only_shutdown, Some(45.0)).observable);

        // No threshold, temperature readable (AMD iGPU on the test machine): no stop, the
        // conservative cap and the "no threshold" warning.
        let t = thermal_policy(GpuThermalThresholds::default(), Some(49.0));
        assert_eq!((t.stop_at_c, t.observable), (None, false));
        assert_eq!(t.max_duration_s(), MAX_S_UNOBSERVED);
        assert_eq!(t.stop_temperature_c(), None);
        assert_eq!(t.start_warning(IT), Some(warn_gpu_no_threshold(IT)));
        // Nothing readable: the "not observable" warning.
        let t = thermal_policy(th, None);
        assert!(!t.observable);
        assert_eq!(t.max_duration_s(), MAX_S_UNOBSERVED);
        assert_eq!(t.stop_temperature_c(), None);
        assert_eq!(t.start_warning(IT), Some(warn_gpu_temperature(IT)));
        assert_eq!(
            Thermal::unobservable().start_warning(IT),
            Some(warn_gpu_temperature(IT))
        );
    }

    fn ready(thermal: Thermal) -> GpuReady {
        GpuReady {
            target: GpuTarget {
                id: RTX.into(),
                name: "RTX".into(),
                vendor_id: 0x10DE,
                device_id: 0x2783,
            },
            exclude_luids: vec![],
            adapter: machine_wgpu()[1].clone(),
            matched_by: MatchKind::Luid,
            thermal,
        }
    }

    #[test]
    fn temperature_watch_stops_near_slowdown_and_on_lost_sensor() {
        let th = GpuThermalThresholds {
            slowdown_c: Some(94.0),
            ..GpuThermalThresholds::default()
        };
        let mut w = GpuWatch::new(&ready(thermal_policy(th, Some(40.0))), false, IT);
        assert_eq!(w.check(1000, (Some(70.0), Some(2600.0), Some(200.0))), None);
        assert_eq!(w.check(2000, (Some(88.9), None, None)), None);
        let stop = w.check(3000, (Some(89.0), None, None)).expect("stop");
        assert!(
            stop.contains("89 °C") && stop.contains("rallentamento 94 °C"),
            "{stop}"
        );
        // Lost sensor on an observed run: stop after TEMP_LOST_CHECKS checks.
        assert_eq!(w.check(4000, (None, None, None)), None);
        assert_eq!(w.check(5000, (None, None, None)), None);
        assert!(w.check(6000, (None, None, None)).is_some());
        let series = w.into_series();
        let ids: Vec<&str> = series.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["gpu_temp_c", "gpu_clock_mhz", "gpu_power_w"]);
        assert_eq!(series[0].points.len(), 3);
        // Unobserved runs never stop on temperature (the duration cap protects them).
        let mut w = GpuWatch::new(&ready(Thermal::unobservable()), false, IT);
        for i in 0..10 {
            assert_eq!(w.check(i, (None, None, None)), None);
        }
        assert!(w.into_series().is_empty());
    }

    #[test]
    fn temperature_watch_wddm_threshold_and_no_threshold() {
        // WDDM warning: stop at the value itself.
        let th = GpuThermalThresholds {
            driver_warning_c: Some(84.0),
            ..GpuThermalThresholds::default()
        };
        let mut w = GpuWatch::new(&ready(thermal_policy(th, Some(40.0))), false, IT);
        assert_eq!(w.check(1000, (Some(83.9), Some(2000.0), None)), None);
        let stop = w.check(2000, (Some(84.0), None, None)).expect("stop");
        assert!(stop.contains("soglia di avviso WDDM 84 °C"), "{stop}");

        // Readable temperature without any threshold (AMD iGPU): series recorded, never a
        // temperature stop, and losing the reading does not stop the run either.
        let mut w = GpuWatch::new(
            &ready(thermal_policy(GpuThermalThresholds::default(), Some(49.0))),
            false,
            IT,
        );
        for i in 0..5 {
            assert_eq!(w.check(i * 1000, (Some(99.0), Some(2200.0), None)), None);
        }
        for i in 5..10 {
            assert_eq!(w.check(i * 1000, (None, None, None)), None);
        }
        let series = w.into_series();
        let ids: Vec<&str> = series.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["gpu_temp_c", "gpu_clock_mhz"]);
        assert_eq!(series[0].points.len(), 5);
    }

    #[test]
    fn outcome_has_headline_first_stability_metrics_and_adapter_note() {
        let mut points: Vec<Point> = (1..=20)
            .map(|s| Point {
                elapsed_ms: s * 1000,
                flops_per_s: 30e12,
            })
            .collect();
        points[18].flops_per_s = 24e12;
        points[19].flops_per_s = 24e12;
        let r = WorkerResult {
            flops_per_invocation: kernel::flops_per_invocation(),
            invocations_per_dispatch: 4 << 20,
            dispatches_per_batch: 8,
            batches: 500,
            total_flops: 29.4e12 * 20.0,
            timed_ms: 20_000,
            warmup_ms: 2_000,
            points,
            verified_elements: 64,
            max_rel_error: 0.0,
        };
        let o = build_outcome(&r, &machine_wgpu()[1], MatchKind::Luid, 2500, IT);
        assert_eq!(o.metrics[0].id, "throughput");
        assert!((o.metrics[0].value - 29.4e12).abs() < 1.0);
        assert_eq!(o.metrics[0].unit, Unit::FlopsPerSecond);
        let drop = o
            .metrics
            .iter()
            .find(|m| m.id == "throughput_drop_percent")
            .expect("drop");
        assert!((drop.value - 20.0).abs() < 1e-9, "{}", drop.value);
        assert!(o.metrics.iter().any(|m| m.id == "throughput_cv"));
        assert_eq!(o.series[0].id, "throughput");
        assert_eq!(o.series[0].unit, Unit::FlopsPerSecond);
        assert_eq!(o.series[0].points[0].elapsed_ms, 3500);
        // The adapter line is a neutral note, not a warning.
        assert_eq!(o.notes.len(), 1);
        assert!(o.notes[0].starts_with("Adattatore GPU utilizzato"));
        assert!(o.notes[0].contains("RTX 4070 SUPER") && o.notes[0].contains("LUID"));
        assert!(
            !o.warnings
                .iter()
                .any(|w| w.contains("Adattatore GPU utilizzato"))
        );
        assert!(o.warnings.iter().any(|w| w.contains("riduzione sostenuta")));

        // A stable run: a note, no warnings.
        let stable = WorkerResult {
            points: (1..=20)
                .map(|s| Point {
                    elapsed_ms: s * 1000,
                    flops_per_s: 30e12,
                })
                .collect(),
            ..r
        };
        let o = build_outcome(&stable, &machine_wgpu()[1], MatchKind::PciIds, 0, IT);
        assert!(o.warnings.is_empty(), "{:?}", o.warnings);
        assert!(o.notes[0].contains("ID PCI"));

        let o = build_outcome(&r, &machine_wgpu()[1], MatchKind::PciIds, 0, EN);
        assert_eq!(o.metrics[0].label, "FP32 throughput (FLOP/s)");
        assert_eq!(
            o.notes[0],
            format!(
                "GPU adapter used: NVIDIA GeForce RTX 4070 SUPER (DX12, PCI 10DE:2783, {RTX}), \
                 selected by PCI ID (LUID not verifiable on this backend)."
            )
        );
        assert!(
            o.warnings[0].starts_with("Possible sustained GPU performance reduction"),
            "{:?}",
            o.warnings
        );
    }

    /// An Apple Silicon Mac: one `IOAccelerator` (vendor 0x106B, no PCI device id) and the
    /// Metal adapter with the same registry id (Metal reports no PCI ids).
    #[test]
    fn apple_gpu_is_matched_by_registry_id() {
        const M5: &str = "ioreg:00000001000004A1";
        let env = decide(
            Ok(vec![info(M5, "Apple M5", 0x106B, 0, false, false)]),
            || {
                Ok(vec![report(
                    "Apple M5",
                    "metal",
                    "integrated",
                    0,
                    0,
                    Some(M5),
                )])
            },
            |_| Thermal::unobservable(),
        );
        let r = env.ready().expect("ready");
        assert_eq!((r.target.id.as_str(), r.matched_by), (M5, MatchKind::Luid));
        assert!(r.exclude_luids.is_empty());
        // No readable GPU temperature: the conservative cap applies.
        assert_eq!(r.thermal.max_duration_s(), MAX_S_UNOBSERVED);
        let o = build_outcome(
            &WorkerResult {
                flops_per_invocation: kernel::flops_per_invocation(),
                invocations_per_dispatch: 1024,
                dispatches_per_batch: 1,
                batches: 2,
                total_flops: 2.0e12,
                timed_ms: 2000,
                warmup_ms: 200,
                points: Vec::new(),
                verified_elements: 64,
                max_rel_error: 0.0,
            },
            &r.adapter,
            r.matched_by,
            0,
            EN,
        );
        assert_eq!(
            o.notes,
            [format!(
                "GPU adapter used: Apple M5 (Metal, {M5}), selected by I/O Kit registry ID."
            )]
        );
        let o = build_outcome(
            &WorkerResult {
                flops_per_invocation: kernel::flops_per_invocation(),
                invocations_per_dispatch: 1024,
                dispatches_per_batch: 1,
                batches: 2,
                total_flops: 2.0e12,
                timed_ms: 2000,
                warmup_ms: 200,
                points: Vec::new(),
                verified_elements: 64,
                max_rel_error: 0.0,
            },
            &r.adapter,
            r.matched_by,
            0,
            IT,
        );
        assert_eq!(
            o.notes,
            [format!(
                "Adattatore GPU utilizzato: Apple M5 (Metal, {M5}), selezionato per ID del \
                 registro I/O Kit."
            )]
        );
    }

    #[test]
    fn english_thermal_texts() {
        let th = GpuThermalThresholds {
            slowdown_c: Some(94.0),
            shutdown_c: Some(101.0),
            ..GpuThermalThresholds::default()
        };
        let t = thermal_policy(th, Some(40.0));
        assert_eq!(thresholds_text(&t, EN), "slowdown 94 °C, shutdown 101 °C");
        assert_eq!(
            thresholds_text(&t, IT),
            "rallentamento 94 °C, spegnimento 101 °C"
        );
        let mut w = GpuWatch::new(&ready(t), false, EN);
        assert_eq!(
            w.check(1000, (Some(89.0), None, None)).as_deref(),
            Some(
                "GPU temperature 89 °C: reached the 89 °C safety limit derived from the \
                 driver's thresholds (slowdown 94 °C, shutdown 101 °C): test stopped."
            )
        );
        let labels: Vec<String> = w.into_series().into_iter().map(|s| s.label).collect();
        assert_eq!(labels, ["GPU temperature"]);
        assert_eq!(
            Thermal::unobservable().start_warning(EN),
            Some("GPU temperature not observable: conservative duration limits applied")
        );
        assert_eq!(phase_text(protocol::phase::MEASURE, EN), "Measuring");
        assert_eq!(phase_text(protocol::phase::MEASURE, IT), "Misura");
    }
}
