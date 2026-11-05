//! CPU workloads (methodology v1).
//!
//! One *work unit* is a fixed, deterministic mix of three kernels:
//! 1. integer: a xorshift64*-driven read-modify-write over a 256-entry table
//!    (data-dependent indexing, multiply, rotate), 8192 steps;
//! 2. hashing: FNV-1a 64 over the 2 KiB table, 4 rounds (8192 byte steps);
//! 3. floating point: Mandelbrot escape iterations on a fixed 32×32 grid, max 64 iterations.
//!
//! The unit result is a pure function of its seed. Each worker first recomputes the
//! reference unit and compares it to the value computed at prepare time, and repeats the
//! check every [`VERIFY_EVERY`] units; a mismatch fails the run. Every result is passed
//! through `black_box` and folded into a per-worker checksum, so nothing can be elided.
//! Throughput is units per second over the measured window (after a 1 s warm-up).

use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use models::benchmark::BenchmarkKind;
use models::benchmark_run::{BenchmarkSeries, SeriesPoint};
use models::settings::Language;
use models::{AppError, ErrorCode, Unit};

use crate::runtime::{Outcome, RunCtx, RunError, join_all, metric};
use crate::stats;

pub(crate) const REFERENCE_SEED: u64 = 0x5EED_0000_0000_0001;
const VERIFY_EVERY: u64 = 1024;
pub(crate) const WARMUP: Duration = Duration::from_secs(1);
/// Drop in throughput (first 10 % → last 10 %, positive = slower) that triggers the
/// sustained-run warning.
pub(crate) const SUSTAINED_DROP_WARN_PERCENT: f64 = 10.0;
const TICK: Duration = Duration::from_millis(20);

#[inline]
fn xorshift64(mut x: u64) -> u64 {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

/// One work unit. Deterministic: the same seed always gives the same result.
pub(crate) fn work_unit(seed: u64) -> u64 {
    // 1. Integer kernel.
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    if x == 0 {
        x = 1;
    }
    let mut table = [0u64; 256];
    for slot in &mut table {
        x = xorshift64(x);
        *slot = x;
    }
    let mut acc = 0u64;
    for _ in 0..8192 {
        x = xorshift64(x);
        let i = (x >> 56) as usize;
        let v = table[i].rotate_left((x & 63) as u32) ^ x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        table[i] = v;
        acc = acc.wrapping_add(v);
    }

    // 2. Hashing kernel (FNV-1a 64).
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for _ in 0..4 {
        for w in &table {
            for b in w.to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01B3);
            }
        }
    }

    // 3. Floating-point kernel (Mandelbrot, fixed grid, seed-dependent sub-pixel offset).
    let offset = (seed % 64) as f64 * 1e-3;
    let mut iterations = 0u64;
    for py in 0..32u32 {
        let ci = -1.25 + 2.5 * f64::from(py) / 32.0;
        for px in 0..32u32 {
            let cr = -2.0 + 2.5 * f64::from(px) / 32.0 + offset;
            let (mut zr, mut zi) = (0.0f64, 0.0f64);
            let mut n = 0u64;
            while n < 64 && zr * zr + zi * zi <= 4.0 {
                let t = zr * zr - zi * zi + cr;
                zi = 2.0 * zr * zi + ci;
                zr = t;
                n += 1;
            }
            iterations += n;
        }
    }

    acc ^ h.rotate_left(17) ^ iterations.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

#[repr(align(64))]
#[derive(Default)]
struct PaddedCounter(AtomicU64);

fn mismatch() -> AppError {
    AppError::new(
        ErrorCode::Internal,
        "CPU work unit produced a result different from the reference value",
    )
    .with_detail("the computation is deterministic; the run was stopped and no result is reported")
}

/// Worker loop: runs units until `stop`/cancel. Returns its checksum, or an error on a
/// reference mismatch. `seed_base` makes the seed sequence distinct per worker.
fn worker(
    ctx: &RunCtx,
    stop: &AtomicBool,
    counter: &AtomicU64,
    reference: u64,
    seed_base: u64,
    inject_panic: bool,
) -> Result<u64, AppError> {
    if black_box(work_unit(black_box(REFERENCE_SEED))) != reference {
        return Err(mismatch());
    }
    assert!(!inject_panic, "injected test panic in CPU worker");
    let mut checksum = 0u64;
    let mut n = 0u64;
    while !stop.load(Ordering::Relaxed) && !ctx.is_cancelled() {
        n += 1;
        let r = black_box(work_unit(black_box(seed_base.wrapping_add(n))));
        checksum = checksum.rotate_left(1) ^ r;
        counter.fetch_add(1, Ordering::Relaxed);
        if n.is_multiple_of(VERIFY_EVERY)
            && black_box(work_unit(black_box(REFERENCE_SEED))) != reference
        {
            return Err(mismatch());
        }
    }
    Ok(checksum)
}

struct Measured {
    throughput: f64,
    per_second: Vec<SeriesPoint>,
    checksum: u64,
}

/// Runs `threads` workers for warm-up + `duration` and measures units/s.
fn measure(
    ctx: &RunCtx,
    threads: u32,
    duration: Duration,
    label: &str,
) -> Result<Measured, RunError> {
    let reference = black_box(work_unit(black_box(REFERENCE_SEED)));
    let stop = AtomicBool::new(false);
    let counters: Vec<PaddedCounter> = (0..threads).map(|_| PaddedCounter::default()).collect();
    let total = || -> u64 { counters.iter().map(|c| c.0.load(Ordering::Relaxed)).sum() };
    let full = WARMUP + duration;

    std::thread::scope(|s| -> Result<Measured, RunError> {
        let mut handles = Vec::with_capacity(threads as usize);
        for (i, c) in counters.iter().enumerate() {
            let stop = &stop;
            let inject = ctx.hooks.panic_in_worker && i == 0;
            let seed_base = (i as u64 + 1) << 40;
            match ctx.spawn_worker(s, &format!("bench-cpu-{i}"), move || {
                worker(ctx, stop, &c.0, reference, seed_base, inject)
            }) {
                Ok(h) => handles.push(h),
                Err(e) => {
                    stop.store(true, Ordering::Relaxed);
                    join_all(handles);
                    return Err(e.into());
                }
            }
        }

        let begin = Instant::now();
        ctx.set_phase(ctx.lang.pick("Riscaldamento", "Warming up"), 0.0);
        let mut measuring: Option<(Instant, u64)> = None;
        let mut last_point: Option<(Instant, u64)> = None;
        let mut per_second = Vec::new();
        let mut end: Option<(Instant, u64)> = None;
        loop {
            std::thread::sleep(TICK);
            let now = Instant::now();
            // A worker that exits early (error or panic) stops the run.
            if ctx.is_cancelled() || handles.iter().any(|h| h.is_finished()) {
                break;
            }
            let elapsed = now - begin;
            ctx.set_fraction(elapsed.as_secs_f64() / full.as_secs_f64());
            if measuring.is_none() && elapsed >= WARMUP {
                let c = total();
                measuring = Some((now, c));
                last_point = Some((now, c));
                ctx.set_phase(label, elapsed.as_secs_f64() / full.as_secs_f64());
            }
            if let (Some((m0, c0)), Some((p0, pc))) = (measuring, last_point) {
                if now - p0 >= Duration::from_secs(1) {
                    let c = total();
                    let rate = (c - pc) as f64 / (now - p0).as_secs_f64();
                    per_second.push(SeriesPoint {
                        elapsed_ms: ctx.elapsed_ms(),
                        value: rate,
                    });
                    last_point = Some((now, c));
                    let so_far = (c - c0) as f64 / (now - m0).as_secs_f64();
                    ctx.set_interim(vec![metric(
                        "throughput",
                        "Throughput",
                        so_far,
                        Unit::OperationsPerSecond,
                        true,
                    )]);
                }
                if now - m0 >= duration {
                    end = Some((now, total()));
                    break;
                }
            }
        }
        stop.store(true, Ordering::Relaxed);
        let results = join_all(handles);
        let mut checksum = 0u64;
        for r in results {
            checksum ^= r?;
        }
        ctx.check()?;
        let (Some((m0, c0)), Some((m1, c1))) = (measuring, end) else {
            return Err(RunError::Failed(AppError::internal(
                "CPU workers stopped before the measurement window ended",
            )));
        };
        let units = c1 - c0;
        if units == 0 {
            return Err(RunError::Failed(AppError::internal(
                "no CPU work unit completed in the measurement window",
            )));
        }
        Ok(Measured {
            throughput: units as f64 / (m1 - m0).as_secs_f64(),
            per_second,
            checksum: black_box(checksum),
        })
    })
}

fn series(points: Vec<SeriesPoint>, lang: Language) -> BenchmarkSeries {
    BenchmarkSeries {
        id: "throughput".into(),
        label: lang
            .pick("Throughput al secondo", "Throughput per second")
            .into(),
        unit: Unit::OperationsPerSecond,
        points,
    }
}

pub(crate) fn run(
    ctx: &RunCtx,
    kind: BenchmarkKind,
    threads: u32,
    duration: Duration,
) -> Result<Outcome, RunError> {
    let threads = if kind == BenchmarkKind::CpuSingleCore {
        1
    } else {
        threads.max(1)
    };
    let lang = ctx.lang;
    let label = match kind {
        BenchmarkKind::CpuSingleCore => lang.pick("Carico single-core", "Single-core load"),
        BenchmarkKind::CpuMultiCore => lang.pick("Carico multi-core", "Multi-core load"),
        _ => lang.pick("Carico prolungato", "Sustained load"),
    };
    let m = measure(ctx, threads, duration, label)?;
    tracing::debug!(checksum = m.checksum, "cpu checksum");
    let mut out = Outcome::default();
    match kind {
        BenchmarkKind::CpuSingleCore => {
            out.metrics.push(metric(
                "throughput",
                lang.pick("Throughput single-core", "Single-core throughput"),
                m.throughput,
                Unit::OperationsPerSecond,
                true,
            ));
        }
        BenchmarkKind::CpuMultiCore => {
            out.metrics.push(metric(
                "throughput",
                lang.pick("Throughput multi-core", "Multi-core throughput"),
                m.throughput,
                Unit::OperationsPerSecond,
                true,
            ));
            out.metrics.push(metric(
                "throughput_per_thread",
                "Throughput per thread",
                m.throughput / f64::from(threads),
                Unit::OperationsPerSecond,
                true,
            ));
        }
        _ => {
            let values: Vec<f64> = m.per_second.iter().map(|p| p.value).collect();
            out.metrics.push(metric(
                "throughput",
                lang.pick("Throughput medio", "Average throughput"),
                m.throughput,
                Unit::OperationsPerSecond,
                true,
            ));
            if let Some(change) = stats::first_last_decile_change_percent(&values) {
                // Positive = slower at the end than at the start.
                let drop = -change;
                out.metrics.push(metric(
                    "throughput_drop_percent",
                    lang.pick(
                        "Calo tra primo e ultimo 10% del test",
                        "Drop between the first and last 10% of the test",
                    ),
                    drop,
                    Unit::Percent,
                    false,
                ));
                if drop >= SUSTAINED_DROP_WARN_PERCENT {
                    out.warnings.push(sustained_drop_warning(drop, lang));
                }
            }
            if let Some(cv) = stats::cv_percent(&values) {
                out.metrics.push(metric(
                    "throughput_cv",
                    lang.pick(
                        "Coefficiente di variazione del throughput",
                        "Throughput coefficient of variation",
                    ),
                    cv,
                    Unit::Percent,
                    false,
                ));
            }
        }
    }
    out.series.push(series(m.per_second, lang));
    Ok(out)
}

fn sustained_drop_warning(drop: f64, lang: Language) -> String {
    lang.pick(
        format!(
            "Possibile riduzione sostenuta delle prestazioni: calo del {drop:.1}% tra il primo \
             e l'ultimo 10% del test. La causa (limite termico, energetico o carico esterno) \
             non è determinabile da questo test."
        ),
        format!(
            "Possible sustained performance reduction: {drop:.1}% drop between the first and \
             last 10% of the test. The cause (thermal or power limit, or outside load) can't be \
             determined by this test."
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden value for methodology v1. If this changes, the workload changed: bump
    /// `methodology_version` for the CPU kinds.
    const GOLDEN_REFERENCE: u64 = 0xFCC6_9579_238C_A50E;

    #[test]
    fn work_unit_is_deterministic_and_seed_dependent() {
        let a = work_unit(REFERENCE_SEED);
        assert_eq!(a, work_unit(REFERENCE_SEED));
        assert_ne!(a, work_unit(REFERENCE_SEED + 1));
        assert_ne!(work_unit(1), work_unit(2));
        assert_eq!(a, GOLDEN_REFERENCE, "methodology v1 golden value: {a:#x}");
    }

    #[test]
    fn work_unit_is_not_elided() {
        // A unit must take measurable time even in an optimized build: if the compiler
        // removed the kernels this would be ~0.
        let n = 200u64;
        let t = Instant::now();
        let mut c = 0u64;
        for i in 0..n {
            c ^= black_box(work_unit(black_box(i)));
        }
        black_box(c);
        let per_unit = t.elapsed() / n as u32;
        assert!(per_unit > Duration::from_micros(5), "{per_unit:?} per unit");
    }

    #[test]
    fn sustained_drop_warning_in_both_languages() {
        assert!(
            sustained_drop_warning(12.34, Language::Italian)
                .starts_with("Possibile riduzione sostenuta delle prestazioni: calo del 12.3%")
        );
        assert_eq!(
            sustained_drop_warning(12.34, Language::English),
            "Possible sustained performance reduction: 12.3% drop between the first and last \
             10% of the test. The cause (thermal or power limit, or outside load) can't be \
             determined by this test."
        );
        assert_eq!(
            series(Vec::new(), Language::English).label,
            "Throughput per second"
        );
    }
}
