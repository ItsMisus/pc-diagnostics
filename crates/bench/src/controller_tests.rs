//! Controller tests against the real machine. Each one is kept short (≲ 3 s).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use models::benchmark::{BenchmarkKind, BenchmarkState};
use models::benchmark_run::{BenchmarkRequest, BenchmarkResult, BenchmarkSeries, SeriesPoint};
use models::settings::Language;
use models::{ErrorCode, Unit};

use crate::controller::sanitize;
use crate::gpu::{self, GpuEnv, Unusable, WorkerCommand};
use crate::gpu_worker::Simulate;
use crate::runtime::{Hooks, metric};
use crate::storage::bench_file_name;
use crate::{BenchController, LatestSample};

fn no_sampler() -> LatestSample {
    Arc::new(|| None)
}

fn hooks() -> Hooks {
    Hooks {
        min_duration_s: Some(1),
        min_file_mib: Some(16),
        prep_window: Some(Duration::from_millis(100)),
        // Never probe the real GPU in generic tests.
        gpu_env: Some(GpuEnv::Unavailable(Unusable::NoUsableGpu)),
        ..Hooks::default()
    }
}

fn controller(h: Hooks) -> BenchController {
    BenchController::with_hooks("test".into(), "debug".into(), no_sampler(), h)
}

fn req(kind: BenchmarkKind) -> BenchmarkRequest {
    BenchmarkRequest {
        kind,
        duration_s: None,
        threads: None,
        storage_volume: None,
        storage_file_mib: None,
        storage_write_consent: false,
        gpu_id: None,
    }
}

fn cpu(kind: BenchmarkKind, threads: Option<u32>, secs: u32) -> BenchmarkRequest {
    BenchmarkRequest {
        duration_s: Some(secs),
        threads,
        ..req(kind)
    }
}

fn system_volume() -> String {
    platform::bench_env::system_volume().expect("system volume")
}

fn storage(kind: BenchmarkKind, mib: u32) -> BenchmarkRequest {
    BenchmarkRequest {
        storage_volume: Some(system_volume()),
        storage_file_mib: Some(mib),
        storage_write_consent: true,
        ..req(kind)
    }
}

struct Finished {
    at: Instant,
    result: BenchmarkResult,
    workers_at_finish: usize,
}

struct Harness {
    ctrl: Arc<BenchController>,
    rx: mpsc::Receiver<Finished>,
    calls: Arc<AtomicUsize>,
    /// States seen in progress callbacks.
    states: Arc<Mutex<Vec<BenchmarkState>>>,
    /// Every applied transition, recorded by the controller.
    transitions: Arc<Mutex<Vec<BenchmarkState>>>,
    phases: Arc<Mutex<Vec<String>>>,
}

const IT: Language = Language::Italian;
const EN: Language = Language::English;

fn start(hk: Hooks, request: BenchmarkRequest) -> (Harness, String) {
    start_in(hk, request, IT)
}

fn start_in(mut hk: Hooks, request: BenchmarkRequest, lang: Language) -> (Harness, String) {
    let transitions = Arc::new(Mutex::new(Vec::new()));
    hk.transitions = Some(Arc::clone(&transitions));
    let ctrl = Arc::new(controller(hk));
    let (tx, rx) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let states = Arc::new(Mutex::new(Vec::new()));
    let phases = Arc::new(Mutex::new(Vec::new()));
    let (c2, s2, p2) = (Arc::clone(&calls), Arc::clone(&states), Arc::clone(&phases));
    // Weak: the callback must not keep the controller alive.
    let weak = Arc::downgrade(&ctrl);
    let id = ctrl
        .start(
            request,
            lang,
            move |p| {
                s2.lock().expect("lock").push(p.state);
                p2.lock().expect("lock").push(p.phase.clone());
                assert!(p.fraction.is_finite() && (0.0..=1.0).contains(&p.fraction));
            },
            move |result| {
                c2.fetch_add(1, Ordering::SeqCst);
                let workers_at_finish = weak.upgrade().map_or(0, |c| c.active_worker_threads());
                let _ = tx.send(Finished {
                    at: Instant::now(),
                    result,
                    workers_at_finish,
                });
            },
        )
        .expect("start");
    (
        Harness {
            ctrl,
            rx,
            calls,
            states,
            transitions,
            phases,
        },
        id,
    )
}

impl Harness {
    fn wait(&self, timeout: Duration) -> Finished {
        self.rx.recv_timeout(timeout).expect("on_finished in time")
    }

    fn wait_running(&self) {
        let t = Instant::now();
        while self.ctrl.status().map(|p| p.state) != Some(BenchmarkState::Running) {
            assert!(
                t.elapsed() < Duration::from_secs(5),
                "never reached Running"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn assert_legal_states(&self, last: BenchmarkState) {
        let mut seq = self.transitions.lock().expect("lock").clone();
        seq.insert(0, BenchmarkState::Idle);
        for w in seq.windows(2) {
            assert!(
                w[0].can_transition_to(w[1]),
                "illegal {:?} -> {:?} in {seq:?}",
                w[0],
                w[1]
            );
        }
        assert_eq!(seq.last(), Some(&last), "{seq:?}");
        for s in self.states.lock().expect("lock").iter() {
            assert!(seq.contains(s), "observed {s:?} not in {seq:?}");
        }
    }
}

#[test]
fn capabilities_cover_every_kind() {
    let c = controller(hooks());
    let caps = c.capabilities(IT);
    assert_eq!(caps.len(), 8);
    let gpu = caps
        .iter()
        .find(|c| c.kind == BenchmarkKind::GpuCompute)
        .expect("gpu");
    assert!(!gpu.availability.is_available());
    assert!(
        gpu.description
            .ends_with("Nessuna GPU fisica utilizzabile.")
    );
    let en = c.capabilities(EN);
    let gpu_en = en
        .iter()
        .find(|c| c.kind == BenchmarkKind::GpuCompute)
        .expect("gpu");
    assert!(gpu_en.description.ends_with("No usable physical GPU."));
    assert!(
        gpu_en
            .description
            .starts_with("FP32 floating-point compute")
    );
    // A ready GPU environment makes it available and names the GPU.
    let c = controller(Hooks {
        gpu_env: Some(gpu::test_env(false)),
        ..hooks()
    });
    let caps = c.capabilities(IT);
    let gpu = caps
        .iter()
        .find(|c| c.kind == BenchmarkKind::GpuCompute)
        .expect("gpu");
    assert_eq!(gpu.availability.value(), Some(&true));
    assert_eq!(gpu.gpu_targets.len(), 1);
    assert_eq!(gpu.gpu_targets[0].name, "Test GPU");
    assert!(gpu.gpu_targets[0].is_default);
    assert_eq!(gpu.max_duration_s, Some(30));
    assert!(
        caps.iter()
            .filter(|c| c.kind != BenchmarkKind::GpuCompute)
            .all(|c| c.gpu_targets.is_empty())
    );
}

#[test]
fn invalid_requests_are_rejected_synchronously() {
    let c = controller(Hooks {
        gpu_env: hooks().gpu_env,
        ..Hooks::default()
    });
    let e = c
        .start(req(BenchmarkKind::GpuCompute), IT, |_| {}, |_| {})
        .expect_err("gpu");
    assert_eq!(e.code, ErrorCode::NotSupported);
    let e = c
        .start(
            cpu(BenchmarkKind::CpuSingleCore, None, 1),
            IT,
            |_| {},
            |_| {},
        )
        .expect_err("too short");
    assert_eq!(e.code, ErrorCode::InvalidInput);
    let mut r = storage(BenchmarkKind::StorageSequential, 1024);
    r.storage_write_consent = false;
    assert_eq!(
        c.start(r, IT, |_| {}, |_| {}).expect_err("consent").code,
        ErrorCode::InvalidInput
    );
    let mut r = storage(BenchmarkKind::StorageSequential, 1024);
    r.storage_volume = Some("Q:\\".into());
    if !platform::bench_env::fixed_volumes().contains(&"Q:\\".to_owned()) {
        assert_eq!(
            c.start(r, IT, |_| {}, |_| {}).expect_err("volume").code,
            ErrorCode::InvalidInput
        );
    }
    // macOS: a mount point that is not a present fixed volume, and paths that are not
    // mount points at all (the data volume, a directory, a device).
    #[cfg(target_os = "macos")]
    for bad in [
        "/Volumes/PCDiagnosticsNoSuchVolume",
        "/System/Volumes/Data",
        "/Users",
        "/dev/disk0",
    ] {
        let mut r = storage(BenchmarkKind::StorageSequential, 1024);
        r.storage_volume = Some(bad.into());
        assert_eq!(
            c.start(r, IT, |_| {}, |_| {}).expect_err("volume").code,
            ErrorCode::InvalidInput,
            "{bad}"
        );
    }
    assert!(c.status().is_none(), "nothing started");
    assert_eq!(
        c.cancel("00000000-0000-0000-0000-000000000000")
            .expect_err("unknown")
            .code,
        ErrorCode::NotFound
    );
}

#[test]
fn single_run_slot_and_prompt_cancellation() {
    let (h, id) = start(hooks(), cpu(BenchmarkKind::CpuMultiCore, Some(2), 60));
    // Second start while active → Busy.
    let e = h
        .ctrl
        .start(
            cpu(BenchmarkKind::CpuSingleCore, None, 5),
            IT,
            |_| {},
            |_| {},
        )
        .expect_err("busy");
    assert_eq!(e.code, ErrorCode::Busy);
    assert_eq!(
        h.ctrl.cancel("not-the-id").expect_err("wrong id").code,
        ErrorCode::NotFound
    );

    h.wait_running();
    std::thread::sleep(Duration::from_millis(300));
    assert!(h.ctrl.active_worker_threads() >= 3, "driver + 2 workers");
    let cancelled_at = Instant::now();
    h.ctrl.cancel(&id).expect("cancel");
    let f = h.wait(Duration::from_secs(5));
    let latency = f.at - cancelled_at;
    eprintln!("cancellation latency: {latency:?}");
    assert!(latency < Duration::from_millis(500), "took {latency:?}");
    assert_eq!(f.result.state, BenchmarkState::Cancelled);
    assert!(f.result.metrics.is_empty());
    assert!(f.result.error.is_none());
    assert!(f.result.safety_stop.is_none());
    assert_eq!(
        f.workers_at_finish, 0,
        "all workers joined before on_finished"
    );
    assert_eq!(f.result.run_id, id);
    // Slot is free again; the old id is gone.
    assert!(h.ctrl.status().is_none());
    assert_eq!(
        h.ctrl.cancel(&id).expect_err("finished").code,
        ErrorCode::NotFound
    );
    h.assert_legal_states(BenchmarkState::Cancelled);
    h.ctrl.shutdown();
    assert_eq!(
        h.calls.load(Ordering::SeqCst),
        1,
        "on_finished exactly once"
    );
}

#[test]
fn cancel_during_preparing_is_legal() {
    // Cancel immediately: lands while the context is being collected (Preparing).
    let (h, id) = start(hooks(), cpu(BenchmarkKind::CpuSingleCore, None, 5));
    h.ctrl.cancel(&id).expect("cancel");
    let f = h.wait(Duration::from_secs(5));
    assert_eq!(f.result.state, BenchmarkState::Cancelled);
    h.assert_legal_states(BenchmarkState::Cancelled);
}

#[test]
fn completed_cpu_run_has_headline_and_series() {
    let (h, _) = start(hooks(), cpu(BenchmarkKind::CpuSingleCore, None, 1));
    let f = h.wait(Duration::from_secs(10));
    let r = &f.result;
    assert_eq!(r.state, BenchmarkState::Completed, "{:?}", r.error);
    assert_eq!(r.metrics[0].id, "throughput");
    assert!(r.metrics[0].value > 0.0);
    assert_eq!(r.metrics[0].unit, Unit::OperationsPerSecond);
    assert!(r.series.iter().any(|s| s.id == "throughput"));
    assert_eq!(r.request.duration_s, Some(1));
    assert_eq!(r.methodology_version, 1);
    assert!(
        r.warnings
            .iter()
            .any(|w| w.contains("Temperatura CPU non osservabile"))
    );
    assert!(r.warnings.iter().any(|w| w.contains("Build di debug")));
    assert!(r.context.logical_processors >= 1);
    assert!(r.context.cpu_brand.is_some());
    assert_eq!(f.workers_at_finish, 0);
    h.assert_legal_states(BenchmarkState::Completed);
    assert!(
        h.phases
            .lock()
            .expect("lock")
            .iter()
            .any(|p| p == "Carico single-core")
    );
    drop(h.ctrl);
    assert_eq!(h.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn english_run_texts() {
    let (h, _) = start_in(hooks(), cpu(BenchmarkKind::CpuSingleCore, None, 1), EN);
    let f = h.wait(Duration::from_secs(10));
    let r = &f.result;
    assert_eq!(r.state, BenchmarkState::Completed, "{:?}", r.error);
    assert_eq!(r.metrics[0].label, "Single-core throughput");
    let series = r
        .series
        .iter()
        .find(|s| s.id == "throughput")
        .expect("series");
    assert_eq!(series.label, "Throughput per second");
    assert!(
        r.warnings
            .iter()
            .any(|w| w == "CPU temperature not observable: conservative duration limits applied")
    );
    assert!(r.warnings.iter().any(|w| w.starts_with("Debug build")));
    assert!(
        r.warnings
            .iter()
            .any(|w| w.starts_with("Live monitoring not running"))
    );
    let phases = h.phases.lock().expect("lock").clone();
    assert!(phases.iter().any(|p| p == "Preparing"), "{phases:?}");
    assert!(phases.iter().any(|p| p == "Single-core load"), "{phases:?}");
}

#[test]
fn worker_panic_fails_the_run() {
    let hk = Hooks {
        panic_in_worker: true,
        ..hooks()
    };
    let (h, _) = start(hk, cpu(BenchmarkKind::CpuMultiCore, Some(2), 5));
    let f = h.wait(Duration::from_secs(5));
    assert_eq!(f.result.state, BenchmarkState::Failed);
    let e = f.result.error.as_ref().expect("error");
    assert_eq!(e.code, ErrorCode::Internal);
    assert!(e.detail.as_deref().unwrap_or("").contains("injected"));
    assert!(f.result.metrics.is_empty());
    assert_eq!(f.workers_at_finish, 0);
    h.assert_legal_states(BenchmarkState::Failed);
    // The controller is usable again.
    let (tx, rx) = mpsc::channel();
    let id = h
        .ctrl
        .start(
            cpu(BenchmarkKind::CpuSingleCore, None, 5),
            IT,
            |_| {},
            move |r| {
                let _ = tx.send(r.state);
            },
        )
        .expect("restart");
    h.ctrl.cancel(&id).expect("cancel");
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).expect("finished"),
        BenchmarkState::Cancelled
    );
}

#[test]
fn shutdown_cancels_and_joins() {
    let (h, _) = start(hooks(), cpu(BenchmarkKind::CpuMultiCore, Some(2), 60));
    h.wait_running();
    let t = Instant::now();
    h.ctrl.shutdown();
    assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
    assert_eq!(h.ctrl.active_worker_threads(), 0);
    let f = h.wait(Duration::from_millis(10));
    assert_eq!(f.result.state, BenchmarkState::Cancelled);
    let e = h
        .ctrl
        .start(
            cpu(BenchmarkKind::CpuSingleCore, None, 5),
            IT,
            |_| {},
            |_| {},
        )
        .expect_err("shut down");
    assert_eq!(e.code, ErrorCode::Internal);
    h.ctrl.shutdown(); // idempotent
    assert_eq!(h.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn memory_bandwidth_completes() {
    let (h, _) = start(hooks(), req(BenchmarkKind::MemoryBandwidth));
    let f = h.wait(Duration::from_secs(30));
    let r = &f.result;
    assert_eq!(r.state, BenchmarkState::Completed, "{:?}", r.error);
    assert_eq!(r.metrics[0].id, "read_bandwidth");
    for id in ["read_bandwidth", "write_bandwidth", "copy_bandwidth"] {
        let m = r.metrics.iter().find(|m| m.id == id).expect(id);
        assert!(m.value > 1e8, "{id} {}", m.value);
    }
    assert_eq!(f.workers_at_finish, 0);
}

/// A fixed volume where a small test file passes the free-space rule: the system volume
/// first (file in %TEMP% / $TMPDIR), otherwise another fixed volume. `None` → the test is skipped.
fn storage_volume() -> Option<String> {
    let sys = platform::bench_env::system_volume();
    let mut vols = platform::bench_env::fixed_volumes();
    vols.sort_by_key(|v| Some(v) != sys.as_ref());
    vols.into_iter().find(|v| {
        platform::bench_env::volume_space(v).is_ok_and(|s| {
            crate::validate::check_free_space(s.free_to_caller_bytes, s.total_bytes, 64 << 20)
                .is_ok()
        })
    })
}

fn test_file_for(volume: &str, id: &str) -> std::path::PathBuf {
    crate::storage::test_dir(
        volume,
        platform::bench_env::system_volume().as_deref(),
        &std::env::temp_dir(),
    )
    .join(bench_file_name(id))
}

fn small_storage(kind: BenchmarkKind) -> Option<(BenchmarkRequest, String)> {
    let Some(vol) = storage_volume() else {
        eprintln!("skipped: no fixed volume satisfies the free-space rule");
        return None;
    };
    let mut r = storage(kind, 16);
    r.storage_volume = Some(vol.clone());
    Some((r, vol))
}

#[test]
fn storage_sequential_cleans_up_its_file() {
    let Some((request, vol)) = small_storage(BenchmarkKind::StorageSequential) else {
        return;
    };
    let (h, id) = start(hooks(), request);
    let f = h.wait(Duration::from_secs(60));
    let r = &f.result;
    assert_eq!(r.state, BenchmarkState::Completed, "{:?}", r.error);
    assert_eq!(r.metrics[0].id, "seq_write");
    let written = r
        .metrics
        .iter()
        .find(|m| m.id == "bytes_written")
        .expect("bytes");
    assert_eq!(written.value, 16.0 * 1024.0 * 1024.0);
    assert!(!test_file_for(&vol, &id).exists(), "test file deleted");
}

#[test]
fn storage_random_cancel_cleans_up_its_file() {
    let hk = Hooks {
        storage_phase: Some(Duration::from_secs(20)),
        ..hooks()
    };
    let Some((request, vol)) = small_storage(BenchmarkKind::StorageRandom) else {
        return;
    };
    let (h, id) = start(hk, request);
    let path = test_file_for(&vol, &id);
    let t = Instant::now();
    while !h
        .phases
        .lock()
        .expect("lock")
        .iter()
        .any(|p| p.contains("casuale"))
    {
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "random phase not reached"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    if cfg!(target_os = "macos") {
        // Unlinked right after it was opened (the macOS equivalent of
        // FILE_FLAG_DELETE_ON_CLOSE): the open file has no name while the run goes on.
        assert!(!path.exists(), "no name while running: {}", path.display());
    } else {
        assert!(
            path.exists(),
            "file exists while running: {}",
            path.display()
        );
    }
    let cancelled_at = Instant::now();
    h.ctrl.cancel(&id).expect("cancel");
    let f = h.wait(Duration::from_secs(5));
    assert!(f.at - cancelled_at < Duration::from_millis(500));
    assert_eq!(f.result.state, BenchmarkState::Cancelled);
    assert!(!path.exists(), "file deleted after cancel");
    assert_eq!(f.workers_at_finish, 0);
}

#[test]
fn non_finite_values_are_dropped() {
    let mut m = vec![
        metric("a", "a", f64::NAN, Unit::Count, true),
        metric("b", "b", 1.0, Unit::Count, true),
        metric("c", "c", f64::INFINITY, Unit::Count, true),
    ];
    let p = |v| SeriesPoint {
        elapsed_ms: 0,
        value: v,
    };
    let mut s = vec![
        BenchmarkSeries {
            id: "x".into(),
            label: "x".into(),
            unit: Unit::Count,
            points: vec![p(1.0), p(f64::NAN)],
        },
        BenchmarkSeries {
            id: "y".into(),
            label: "y".into(),
            unit: Unit::Count,
            points: vec![p(f64::NEG_INFINITY)],
        },
    ];
    sanitize(&mut m, &mut s);
    assert_eq!(m.len(), 1);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].points.len(), 1);
}

// ---------------------------------------------------------------------------------------
// GPU: the worker process is the test binary itself running a simulated mode.
// ---------------------------------------------------------------------------------------

fn gpu_hooks(sim: Simulate) -> (Hooks, Arc<Mutex<Vec<u32>>>) {
    let pids = Arc::new(Mutex::new(Vec::new()));
    let hk = Hooks {
        gpu_env: Some(gpu::test_env(false)),
        gpu_worker: Some(WorkerCommand::test_binary()),
        gpu_simulate: Some(sim),
        gpu_warmup: Some(Duration::from_millis(200)),
        gpu_pids: Some(Arc::clone(&pids)),
        ..hooks()
    };
    (hk, pids)
}

fn gpu_req(secs: u32) -> BenchmarkRequest {
    BenchmarkRequest {
        duration_s: Some(secs),
        ..req(BenchmarkKind::GpuCompute)
    }
}

fn process_alive(pid: u32) -> bool {
    platform::process::ProcessEnumerator::new()
        .scan()
        .expect("process scan")
        .processes
        .iter()
        .any(|p| p.pid == pid)
}

/// Every worker that was started is gone (reaped) when `on_finished` has run.
fn assert_no_worker_left(pids: &Mutex<Vec<u32>>) {
    let pids = pids.lock().expect("lock").clone();
    assert!(!pids.is_empty(), "worker never said hello");
    for pid in pids {
        assert!(!process_alive(pid), "worker {pid} still running");
    }
}

fn wait_for_hello(pids: &Mutex<Vec<u32>>) {
    let t = Instant::now();
    while pids.lock().expect("lock").is_empty() {
        assert!(
            t.elapsed() < Duration::from_secs(10),
            "worker never said hello"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn gpu_simulated_run_completes_through_the_worker_process() {
    let (hk, pids) = gpu_hooks(Simulate::Fake);
    let (h, _) = start(hk, gpu_req(2));
    let f = h.wait(Duration::from_secs(20));
    let r = &f.result;
    assert_eq!(r.state, BenchmarkState::Completed, "{:?}", r.error);
    assert_eq!(r.metrics[0].id, "throughput");
    assert_eq!(r.metrics[0].unit, Unit::FlopsPerSecond);
    assert!((r.metrics[0].value - crate::gpu_worker::FAKE_FLOPS).abs() < 1.0);
    assert!(r.metrics.iter().any(|m| m.id == "throughput_drop_percent"));
    assert!(r.series.iter().any(|s| s.id == "throughput"));
    // The adapter line is a note, not a warning.
    assert!(
        r.notes
            .iter()
            .any(|w| w.contains("Adattatore GPU utilizzato")),
        "{:?}",
        r.notes
    );
    assert!(
        !r.warnings
            .iter()
            .any(|w| w.contains("Adattatore GPU utilizzato"))
    );
    assert!(
        r.warnings
            .iter()
            .any(|w| w == gpu::warn_gpu_temperature(IT))
    );
    assert_eq!(r.request.duration_s, Some(2));
    assert_eq!(r.request.gpu_id.as_deref(), Some(gpu::TEST_GPU_ID));
    assert_eq!(r.methodology_version, 1);
    h.assert_legal_states(BenchmarkState::Completed);
    assert_no_worker_left(&pids);
}

#[test]
fn gpu_selected_target_is_used_and_its_thermal_warning_reported() {
    let (mut hk, pids) = gpu_hooks(Simulate::Fake);
    hk.gpu_env = Some(gpu::test_env_two(true));
    let c = controller(hk.clone());
    let caps = c.capabilities(IT);
    let cap = caps
        .iter()
        .find(|c| c.kind == BenchmarkKind::GpuCompute)
        .expect("gpu");
    assert_eq!(cap.gpu_targets.len(), 2);
    assert_eq!(cap.gpu_targets[0].stop_temperature_c, Some(89.0));
    assert_eq!(cap.gpu_targets[1].stop_temperature_c, None);
    drop(c);
    let r = BenchmarkRequest {
        gpu_id: Some(gpu::TEST_IGPU_ID.into()),
        ..gpu_req(2)
    };
    let (h, _) = start(hk, r);
    let f = h.wait(Duration::from_secs(20));
    let r = &f.result;
    assert_eq!(r.state, BenchmarkState::Completed, "{:?}", r.error);
    assert_eq!(r.request.gpu_id.as_deref(), Some(gpu::TEST_IGPU_ID));
    // Readable temperature without a driver threshold: the "no threshold" warning.
    assert!(
        r.warnings
            .iter()
            .any(|w| w == gpu::warn_gpu_no_threshold(IT)),
        "{:?}",
        r.warnings
    );
    assert!(
        !r.warnings
            .iter()
            .any(|w| w == gpu::warn_gpu_temperature(IT))
    );
    assert!(!r.notes.is_empty());
    assert_no_worker_left(&pids);
}

#[test]
fn gpu_worker_crash_fails_the_run() {
    for sim in [Simulate::Crash, Simulate::ExitSilently] {
        let (hk, pids) = gpu_hooks(sim);
        let (h, _) = start(hk, gpu_req(5));
        let f = h.wait(Duration::from_secs(20));
        let r = &f.result;
        assert_eq!(r.state, BenchmarkState::Failed, "{sim:?}");
        let e = r.error.as_ref().expect("error");
        assert_eq!(e.message, gpu::ABNORMAL_EXIT, "{sim:?}");
        eprintln!("{sim:?}: {:?}", e.detail);
        assert!(r.metrics.is_empty());
        h.assert_legal_states(BenchmarkState::Failed);
        assert_no_worker_left(&pids);
    }
}

#[test]
fn gpu_cancel_stops_the_worker_promptly() {
    for sim in [Simulate::Fake, Simulate::IgnoreCancel] {
        let (hk, pids) = gpu_hooks(sim);
        let (h, id) = start(hk, gpu_req(20));
        h.wait_running();
        wait_for_hello(&pids);
        std::thread::sleep(Duration::from_millis(200));
        // One heavy run at a time.
        let e = h
            .ctrl
            .start(gpu_req(5), IT, |_| {}, |_| {})
            .expect_err("busy");
        assert_eq!(e.code, ErrorCode::Busy);
        let cancelled_at = Instant::now();
        h.ctrl.cancel(&id).expect("cancel");
        let f = h.wait(Duration::from_secs(10));
        let latency = f.at - cancelled_at;
        eprintln!("{sim:?}: GPU cancel latency {latency:?}");
        // Cooperative worker: immediate. Ignoring worker: killed after the 1 s grace.
        assert!(
            latency < Duration::from_millis(1500),
            "{sim:?} took {latency:?}"
        );
        assert_eq!(
            f.result.state,
            BenchmarkState::Cancelled,
            "{:?}",
            f.result.error
        );
        assert!(f.result.error.is_none());
        h.assert_legal_states(BenchmarkState::Cancelled);
        assert_no_worker_left(&pids);
    }
}

#[test]
fn gpu_watchdog_kills_a_runaway_worker() {
    let (mut hk, pids) = gpu_hooks(Simulate::IgnoreCancel);
    hk.hard_limit = Some(Duration::from_millis(1500));
    let (h, _) = start(hk, gpu_req(20));
    let f = h.wait(Duration::from_secs(15));
    let r = &f.result;
    assert_eq!(r.state, BenchmarkState::Cancelled);
    assert!(
        r.safety_stop
            .as_deref()
            .is_some_and(|s| s.contains("Durata massima")),
        "{:?}",
        r.safety_stop
    );
    h.assert_legal_states(BenchmarkState::Cancelled);
    assert_no_worker_left(&pids);
}

#[test]
fn gpu_hung_worker_is_killed_and_fails() {
    let (mut hk, pids) = gpu_hooks(Simulate::Hang);
    hk.gpu_stall = Some(Duration::from_secs(1));
    let t = Instant::now();
    let (h, _) = start(hk, gpu_req(20));
    let f = h.wait(Duration::from_secs(15));
    eprintln!("hung worker failed after {:?}", t.elapsed());
    let r = &f.result;
    assert_eq!(r.state, BenchmarkState::Failed);
    let e = r.error.as_ref().expect("error");
    assert_eq!(e.message, gpu::ABNORMAL_EXIT);
    assert!(
        e.detail
            .as_deref()
            .is_some_and(|d| d.contains("no message"))
    );
    assert_no_worker_left(&pids);
}
