//! Real-machine probe: measured cost of each part of a process tick, then a top-5.
//!
//! `cargo run -p processes --example process_probe [--release] -- [interval_ms] [ticks]`

use std::time::{Duration, Instant};

use models::process::ProcessRow;
use platform::process::{ProcessEnumerator, image_access, memory_status, visible_window_pids};
use processes::{IO_REFERENCE_BYTES_PER_SEC, ScanInput, Tracker, TrackerConfig};

fn stats(name: &str, v: &mut [Duration]) {
    v.sort();
    println!(
        "  {name:<18} min {:>9.3?}  median {:>9.3?}  max {:>9.3?}",
        v[0],
        v[v.len() / 2],
        v[v.len() - 1]
    );
}

fn show(r: &ProcessRow) {
    println!(
        "  {:<28} impact {:>6.2} (cpu {:>6.2} ram {:>6.2} io {:>6.2}) peak {:>6.2} {:?}{}{}",
        r.name,
        r.pressure.impact,
        r.pressure.cpu,
        r.pressure.ram,
        r.pressure.io,
        r.impact_peak,
        r.category,
        if r.accessible { "" } else { " [inaccessible]" },
        if r.is_self { " [self]" } else { "" },
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let interval_ms: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(2_000);
    let ticks: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(10);

    let mut tracker = Tracker::new(TrackerConfig {
        interval_ms: u32::try_from(interval_ms).unwrap_or(2_000),
        window_s: 60,
        history_points: 150,
        logical_processors: u32::try_from(platform::overhead::logical_cpu_count()).unwrap_or(1),
        io_reference_bytes_per_sec: IO_REFERENCE_BYTES_PER_SEC,
        self_pid: std::process::id(),
        system_dir: platform::process::system_directory().map(|d| d.to_ascii_lowercase()),
        max_tracked: processes::MAX_TRACKED_PROCESSES,
    });
    let mut e = ProcessEnumerator::new();
    let (mut scan_t, mut win_t, mut mem_t, mut ingest_t, mut snap_t, mut total_t) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut first_tick = None;
    let mut last = None;
    for i in 0..=ticks {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(interval_ms));
        }
        let t_all = Instant::now();
        let t = Instant::now();
        let scan = e.scan().expect("scan");
        let d_scan = t.elapsed();
        let t = Instant::now();
        let windows = visible_window_pids().ok();
        let d_win = t.elapsed();
        let t = Instant::now();
        let memory = memory_status().ok();
        let d_mem = t.elapsed();
        let t = Instant::now();
        tracker.ingest(
            &ScanInput {
                scan: &scan,
                memory,
                window_pids: windows.as_ref(),
            },
            image_access,
        );
        let d_ingest = t.elapsed();
        let t = Instant::now();
        let snap = tracker.snapshot(10);
        let d_snap = t.elapsed();
        let d_all = t_all.elapsed();
        if i == 0 {
            // Baseline: includes one path lookup per process (cached afterwards).
            first_tick = Some((d_all, d_ingest, scan.processes.len()));
            continue;
        }
        scan_t.push(d_scan);
        win_t.push(d_win);
        mem_t.push(d_mem);
        ingest_t.push(d_ingest);
        snap_t.push(d_snap);
        total_t.push(d_all);
        last = snap;
    }
    if let Some((all, ingest, n)) = first_tick {
        println!(
            "== first (baseline) tick: {all:?} total, {ingest:?} ingest incl. path lookups, {n} entries"
        );
    }
    println!("== steady-state tick cost over {ticks} ticks at {interval_ms} ms:");
    stats("NtQuerySystemInfo", &mut scan_t);
    stats("EnumWindows", &mut win_t);
    stats("memory status", &mut mem_t);
    stats("ingest", &mut ingest_t);
    stats("snapshot (top 10)", &mut snap_t);
    stats("TOTAL", &mut total_t);
    let s = last.expect("snapshot");
    println!(
        "== processes: total {} | inaccessible {} | tracked {} | window {} s",
        s.total_processes,
        s.inaccessible_processes,
        tracker.tracked_count(),
        s.window_s
    );
    println!(
        "== context: {} logical, {:.1} GiB total, {:.1} GiB available",
        s.context.logical_processors,
        s.context.total_memory_bytes / 1_073_741_824.0,
        s.context.available_memory_bytes / 1_073_741_824.0
    );
    println!("== top 5 instant:");
    s.ranking_instant.iter().take(5).for_each(show);
    println!("== top 5 window:");
    s.ranking_window.iter().take(5).for_each(show);
}
