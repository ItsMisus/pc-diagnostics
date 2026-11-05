//! Runs the real GPU compute benchmark on this machine (isolated worker process = this
//! example binary) and prints the capability, the result, the GPU temperature trend and
//! whether any child process is left behind.
//!
//! `cargo run --release -p bench --example gpu_bench_probe [duration_s] [cancel_after_ms]`
//!
//! The GPU is the default target unless `BENCH_GPU` names one: a `GpuInfo::id`
//! (`luid:...`) or a case-insensitive substring of a target name (e.g. `radeon`).
//! `cancel_after_ms` = `-` runs without cancelling.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bench::{BenchController, LatestSample};
use models::benchmark::BenchmarkKind;
use models::benchmark_run::BenchmarkRequest;
use models::settings::Language;
use monitor::{Monitor, MonitorConfig};

fn children_of_self() -> Vec<(u32, String)> {
    let me = std::process::id();
    platform::process::ProcessEnumerator::new()
        .scan()
        .map(|s| {
            s.processes
                .into_iter()
                .filter(|p| p.parent_pid == me)
                .map(|p| (p.pid, p.name))
                .collect()
        })
        .unwrap_or_default()
}

fn main() {
    // The worker process is this same binary.
    if let Some(code) = bench::gpu_worker::run_if_requested() {
        std::process::exit(code);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let duration_s: Option<u32> = args.first().and_then(|a| a.parse().ok());
    let cancel_after: Option<u64> = args.get(1).and_then(|a| a.parse().ok());

    let monitor = Arc::new(Monitor::start(MonitorConfig::default(), |_| {}).expect("monitor"));
    let m2 = Arc::clone(&monitor);
    let latest: LatestSample = Arc::new(move || m2.latest());
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let ctrl = BenchController::new("probe".into(), profile.into(), latest);

    let t = Instant::now();
    let caps = ctrl.capabilities(Language::English);
    println!("capabilities() incl. GPU probe: {:?}", t.elapsed());
    let cap = caps
        .iter()
        .find(|c| c.kind == BenchmarkKind::GpuCompute)
        .expect("gpu capability");
    println!("GpuCompute availability: {:?}", cap.availability);
    println!("  description: {}", cap.description);
    println!(
        "  duration default/max: {:?}/{:?} s, methodology v{}",
        cap.default_duration_s, cap.max_duration_s, cap.methodology_version
    );
    for t in &cap.gpu_targets {
        println!(
            "  target {} \"{}\" default={} available={:?} stop={:?} °C",
            t.id, t.name, t.is_default, t.availability, t.stop_temperature_c
        );
    }
    println!("children after probe: {:?}", children_of_self());
    if !cap.availability.is_available() {
        return;
    }
    let gpu_id = std::env::var("BENCH_GPU").ok().map(|want| {
        let w = want.to_lowercase();
        cap.gpu_targets
            .iter()
            .find(|t| t.id.eq_ignore_ascii_case(&want) || t.name.to_lowercase().contains(&w))
            .map_or(want, |t| t.id.clone())
    });
    println!("requested gpu_id: {gpu_id:?}");

    // Let the monitor collect a few samples (GPU telemetry through NVML when available).
    std::thread::sleep(Duration::from_secs(2));
    let request = BenchmarkRequest {
        kind: BenchmarkKind::GpuCompute,
        duration_s,
        threads: None,
        storage_volume: None,
        storage_file_mib: None,
        storage_write_consent: false,
        gpu_id,
    };
    let (tx, rx) = mpsc::channel();
    let last = Arc::new(Mutex::new(String::new()));
    let l2 = Arc::clone(&last);
    let id = ctrl
        .start(
            request,
            Language::English,
            move |p| {
                let s = format!(
                    "{:?} {:>5.1}% {} {}",
                    p.state,
                    p.fraction * 100.0,
                    p.phase,
                    p.interim
                        .first()
                        .map(|m| format!("{:.0} GFLOP/s", m.value / 1e9))
                        .unwrap_or_default()
                );
                let mut g = l2.lock().expect("lock");
                if *g != s {
                    *g = s.clone();
                    println!("  {s}");
                }
            },
            move |r| {
                let _ = tx.send((Instant::now(), r));
            },
        )
        .expect("start");
    let t0 = Instant::now();
    let mut cancel_at = None;
    if let Some(ms) = cancel_after {
        std::thread::sleep(Duration::from_millis(ms));
        println!("children before cancel: {:?}", children_of_self());
        cancel_at = Some(Instant::now());
        ctrl.cancel(&id).expect("cancel");
    }
    let (at, r) = rx.recv_timeout(Duration::from_secs(200)).expect("finished");
    if let Some(c) = cancel_at {
        println!("cancel -> on_finished: {:?}", at - c);
    }
    println!(
        "== {:?}: {:?} in {:.1} s (wall {:.1} s, methodology v{}, build {})",
        r.kind,
        r.state,
        r.duration_ms as f64 / 1000.0,
        t0.elapsed().as_secs_f64(),
        r.methodology_version,
        r.build_profile
    );
    if let Some(e) = &r.error {
        println!("  error: {} ({:?})", e.message, e.detail);
    }
    if let Some(s) = &r.safety_stop {
        println!("  safety stop: {s}");
    }
    for m in &r.metrics {
        let v = if m.id == "throughput" {
            format!("{:.1} GFLOP/s", m.value / 1e9)
        } else {
            format!("{:.2} %", m.value)
        };
        println!("  {} [{}] = {v}", m.label, m.id);
    }
    for s in &r.series {
        let pts: Vec<String> = s
            .points
            .iter()
            .map(|p| {
                if s.id == "throughput" {
                    format!("{:.0}", p.value / 1e9)
                } else {
                    format!("{:.0}", p.value)
                }
            })
            .collect();
        println!(
            "  series {} ({} pts): {}",
            s.id,
            s.points.len(),
            pts.join(" ")
        );
    }
    for w in &r.warnings {
        println!("  warning: {w}");
    }
    for n in &r.notes {
        println!("  note: {n}");
    }
    println!(
        "  context gpu: {:?} driver {:?}",
        r.context.gpu_name, r.context.gpu_driver_version
    );
    println!("  request: {:?}", r.request);
    std::thread::sleep(Duration::from_millis(200));
    println!("children after run: {:?}", children_of_self());
    ctrl.shutdown();
}
