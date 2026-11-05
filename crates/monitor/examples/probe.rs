//! Real-hardware probe: prints the inventory, one live sample, the app overhead and the
//! measured cost of `LiveCollector::sample()`.
//!
//! `cargo run -p monitor --example probe [--release] -- [interval_ms] [count]`

use std::time::{Duration, Instant};

use models::metric::Metric;
use platform::live::LiveCollector;

fn show<T: std::fmt::Debug>(name: &str, m: &Metric<T>) {
    match m {
        Metric::Available {
            value,
            unit,
            source,
            ..
        } => println!("  {name:<22} = {value:?} {unit:?} [{source:?}]"),
        Metric::Unavailable { reason } => println!("  {name:<22} : UNAVAILABLE {reason:?}"),
    }
}

fn main() {
    let t = Instant::now();
    let inv = platform::inventory::collect_inventory();
    println!("== inventory ({} ms)", t.elapsed().as_millis());
    println!(
        "{}",
        serde_json::to_string_pretty(&inv).unwrap_or_else(|e| e.to_string())
    );

    let t = Instant::now();
    let mut c = LiveCollector::new();
    println!("== LiveCollector::new: {} ms", t.elapsed().as_millis());
    let t = Instant::now();
    let mut o = platform::overhead::OverheadTracker::new();
    println!("== OverheadTracker::new: {} ms", t.elapsed().as_millis());

    // Optional args: <interval ms> <sample count>.
    let mut args = std::env::args().skip(1);
    let interval_ms: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(250);
    let count: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(20);
    let mut costs = Vec::new();
    let mut last = None;
    let mut avail = [0u32; 6];
    for seq in 1..=count {
        std::thread::sleep(Duration::from_millis(interval_ms));
        let t = Instant::now();
        let s = c.sample(seq);
        costs.push(t.elapsed());
        avail[0] += u32::from(s.cpu.usage_percent.is_available());
        avail[1] += u32::from(s.cpu.per_core_percent.is_available());
        avail[2] += u32::from(s.cpu.current_mhz.is_available());
        avail[3] += u32::from(s.memory.pages_per_sec.is_available());
        avail[4] += u32::from(s.storage.active_time_percent.is_available());
        avail[5] += u32::from(s.gpus.iter().any(|g| g.usage_percent.is_available()));
        last = Some(s);
    }
    println!(
        "== availability over {count} samples: cpu.usage {} | cpu.per_core {} | cpu.current_mhz {} | \
         mem.pages_per_sec {} | disk.active {} | any gpu.usage {}",
        avail[0], avail[1], avail[2], avail[3], avail[4], avail[5]
    );
    costs.sort();
    println!(
        "== sample() cost over {} calls: min {:?}  median {:?}  max {:?}",
        costs.len(),
        costs[0],
        costs[costs.len() / 2],
        costs[costs.len() - 1]
    );

    if let Some(s) = last {
        println!("== live sample #{}", s.seq);
        show("cpu.usage", &s.cpu.usage_percent);
        match s.cpu.per_core_percent.value() {
            Some(v) => println!("  cpu.per_core           = {} cores", v.len()),
            None => show("cpu.per_core", &s.cpu.per_core_percent),
        }
        show("cpu.current_mhz", &s.cpu.current_mhz);
        show("cpu.max_mhz", &s.cpu.max_mhz);
        show("cpu.limit_mhz", &s.cpu.limit_mhz);
        show("cpu.temperature_c", &s.cpu.temperature_c);
        show("cpu.power_w", &s.cpu.power_w);
        show("mem.total", &s.memory.total_bytes);
        show("mem.used", &s.memory.used_bytes);
        show("mem.available", &s.memory.available_bytes);
        show("mem.commit_used", &s.memory.commit_used_bytes);
        show("mem.commit_limit", &s.memory.commit_limit_bytes);
        show("mem.pages_per_sec", &s.memory.pages_per_sec);
        for g in &s.gpus {
            println!("  gpu {}", g.id);
            show("  usage", &g.usage_percent);
            show("  dedicated_used", &g.dedicated_used_bytes);
            show("  temperature_c", &g.temperature_c);
            show("  core_clock_mhz", &g.core_clock_mhz);
            show("  power_w", &g.power_w);
            show("  power_percent", &g.power_percent);
            show("  fan_rpm", &g.fan_rpm);
        }
        show("disk.read", &s.storage.read_bytes_per_sec);
        show("disk.write", &s.storage.write_bytes_per_sec);
        show("disk.active", &s.storage.active_time_percent);
    }

    let t = Instant::now();
    let ov = o.measure().clone();
    println!("== overhead measure: {:?}", t.elapsed());
    println!("  window_ms = {}", ov.window_ms);
    show("core.cpu", &ov.core.cpu_percent);
    show("core.ws", &ov.core.working_set_bytes);
    println!("  core.processes = {}", ov.core.process_count);
    show("webview.cpu", &ov.webview.cpu_percent);
    show("webview.ws", &ov.webview.working_set_bytes);
    println!("  webview.processes = {}", ov.webview.process_count);

    // Steady-state cost of the whole sampler at 1 s: this process does nothing else, so
    // the core CPU share reported by the monitor's own overhead window is the sampler's.
    drop(c);
    let m = monitor::Monitor::start(
        monitor::MonitorConfig {
            interval_ms: 1_000,
            history_capacity: 60,
        },
        |_| {},
    )
    .expect("start monitor");
    std::thread::sleep(Duration::from_millis(11_500));
    let ov = m.app_overhead();
    println!(
        "== Monitor at 1 s for 11.5 s: {} samples, overhead window {} ms",
        m.status().samples_collected,
        ov.window_ms
    );
    show("process cpu (machine %)", &ov.core.cpu_percent);
    show("process working set", &ov.core.working_set_bytes);
    m.shutdown();
}
