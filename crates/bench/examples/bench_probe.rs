//! Runs benchmarks at their default settings on this machine and prints the results.
//! Storage tests write a temporary file (explicit consent here) on the system volume (`C:\`
//! on Windows, `/` on macOS) or on the volume named by the `BENCH_VOLUME` environment
//! variable.
//!
//! `cargo run --release -p bench --example bench_probe [kind ...]`
//! kinds: cpu1 cpun cpus memb meml seq rand cancel (default: all)

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bench::{BenchController, LatestSample};
use models::Unit;
use models::benchmark::BenchmarkKind;
use models::benchmark_run::{BenchmarkRequest, BenchmarkResult};
use models::settings::Language;
use monitor::{Monitor, MonitorConfig};

fn req(kind: BenchmarkKind) -> BenchmarkRequest {
    let storage = matches!(
        kind,
        BenchmarkKind::StorageSequential | BenchmarkKind::StorageRandom
    );
    BenchmarkRequest {
        kind,
        duration_s: None,
        threads: None,
        storage_volume: storage.then(|| {
            std::env::var("BENCH_VOLUME")
                .ok()
                .or_else(platform::bench_env::system_volume)
                .unwrap_or_else(|| "C:\\".to_owned())
        }),
        storage_file_mib: std::env::var("BENCH_FILE_MIB")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|_| storage),
        storage_write_consent: storage,
        gpu_id: None,
    }
}

fn fmt(v: f64, unit: Unit) -> String {
    match unit {
        Unit::BytesPerSecond => format!("{:.2} GB/s ({:.0} MB/s)", v / 1e9, v / 1e6),
        Unit::Bytes => format!("{:.0} MiB", v / 1048576.0),
        Unit::OperationsPerSecond => format!("{v:.0} /s"),
        Unit::Nanoseconds => format!("{v:.2} ns"),
        Unit::Microseconds => format!("{v:.1} µs"),
        Unit::Percent => format!("{v:.2} %"),
        _ => format!("{v}"),
    }
}

fn run(ctrl: &BenchController, request: BenchmarkRequest) -> BenchmarkResult {
    let (tx, rx) = mpsc::channel();
    let ticks = Arc::new(Mutex::new(Vec::<Instant>::new()));
    let t2 = Arc::clone(&ticks);
    ctrl.start(
        request,
        Language::English,
        move |_| t2.lock().expect("lock").push(Instant::now()),
        move |r| {
            let _ = tx.send(r);
        },
    )
    .expect("start");
    let r = rx
        .recv_timeout(Duration::from_secs(1500))
        .expect("finished");
    let t = ticks.lock().expect("lock");
    let gaps: Vec<u128> = t.windows(2).map(|w| (w[1] - w[0]).as_millis()).collect();
    let max_gap = gaps.iter().max().copied().unwrap_or(0);
    println!("  progress callbacks: {} (max gap {max_gap} ms)", t.len());
    r
}

fn print(r: &BenchmarkResult) {
    println!(
        "== {:?}: {:?} in {:.1} s (methodology v{}, build {})",
        r.kind,
        r.state,
        r.duration_ms as f64 / 1000.0,
        r.methodology_version,
        r.build_profile
    );
    let c = &r.context;
    println!(
        "  context: {:?} | {:?} | power {:?} plan {:?} | bg CPU {:?} | limit {:?} max {:?} | disk {:?}",
        c.cpu_brand,
        c.os_version,
        c.power_source,
        c.power_plan,
        c.background_cpu_percent.value(),
        c.cpu_limit_mhz.value(),
        c.cpu_max_mhz.value(),
        c.storage_model
    );
    println!("  request: {:?}", r.request);
    for m in &r.metrics {
        println!("  {:<24} {:<45} {}", m.id, m.label, fmt(m.value, m.unit));
    }
    for s in &r.series {
        let vals: Vec<String> = s
            .points
            .iter()
            .take(8)
            .map(|p| format!("{:.0}", p.value))
            .collect();
        println!(
            "  series {} ({} pts): {} …",
            s.id,
            s.points.len(),
            vals.join(", ")
        );
    }
    for w in &r.warnings {
        println!("  warning: {w}");
    }
    if let Some(s) = &r.safety_stop {
        println!("  SAFETY STOP: {s}");
    }
    if let Some(e) = &r.error {
        println!("  ERROR: {e} {:?}", e.detail);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let want = |k: &str| args.is_empty() || args.iter().any(|a| a == k);
    let monitor = Arc::new(Monitor::start(MonitorConfig::default(), |_| {}).expect("monitor"));
    let m2 = Arc::clone(&monitor);
    let latest: LatestSample = Arc::new(move || m2.latest());
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let ctrl = BenchController::new("probe".into(), profile.into(), latest);
    // Let the sampler produce a few samples first (background-load context).
    std::thread::sleep(Duration::from_secs(3));

    let kinds = [
        ("cpu1", BenchmarkKind::CpuSingleCore),
        ("cpun", BenchmarkKind::CpuMultiCore),
        ("cpus", BenchmarkKind::CpuSustained),
        ("memb", BenchmarkKind::MemoryBandwidth),
        ("meml", BenchmarkKind::MemoryLatency),
        ("seq", BenchmarkKind::StorageSequential),
        ("rand", BenchmarkKind::StorageRandom),
    ];
    for (name, kind) in kinds {
        if want(name) {
            let r = run(&ctrl, req(kind));
            print(&r);
            // Idle gap between heavy runs.
            std::thread::sleep(Duration::from_secs(3));
        }
    }

    if want("cancel") {
        for _ in 0..3 {
            let (tx, rx) = mpsc::channel();
            let id = ctrl
                .start(
                    req(BenchmarkKind::CpuMultiCore),
                    Language::English,
                    |_| {},
                    move |r| {
                        let _ = tx.send((Instant::now(), r.state));
                    },
                )
                .expect("start");
            std::thread::sleep(Duration::from_millis(2500));
            let t = Instant::now();
            ctrl.cancel(&id).expect("cancel");
            let (at, state) = rx.recv_timeout(Duration::from_secs(5)).expect("finished");
            println!(
                "cancel latency (multi-core, all threads): {:?} -> {state:?}, workers left {}",
                at - t,
                ctrl.active_worker_threads()
            );
        }
    }
    ctrl.shutdown();
}
