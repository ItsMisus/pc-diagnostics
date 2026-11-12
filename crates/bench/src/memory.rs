//! Memory workloads (methodology v1): multithreaded bandwidth and pointer-chase latency.
//!
//! Buffers are plain `Vec<u64>`s allocated with `try_reserve_exact` (allocation failure is
//! an error, never an abort) and filled in 64 MiB steps with cancellation checks, which
//! also touches every page before timing. They are freed on every exit path by ordinary
//! drop, unwinding included.

use std::hint::black_box;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use models::settings::Language;
use models::{AppError, ErrorCode, Unit};

use crate::runtime::{Outcome, RunCtx, RunError, join_all, metric};
use crate::stats;
use crate::validate::{GIB, MIB};

pub(crate) const BANDWIDTH_MAX_BUFFER: u64 = GIB;
/// Must exceed the last-level cache (64 MiB on the 7900X) by a wide margin.
pub(crate) const MIN_BUFFER: u64 = 256 * MIB;
pub(crate) const LATENCY_BUFFER: u64 = 256 * MIB;
pub(crate) const LATENCY_CACHE_REF: u64 = 32 * 1024;
/// Timed passes per operation (read, write, copy).
pub(crate) const PASSES: usize = 10;
/// Pointer-chase batch and total time budget.
const CHASE_BATCH: usize = 1 << 20;
const CHASE_INNER: usize = 1 << 16;
const CHASE_BUDGET: Duration = Duration::from_secs(2);
const CHASE_MAX_BATCHES: usize = 40;
/// Granularity of cancellation checks inside a pass (u64 elements = 1 MiB).
const SUB_CHUNK: usize = (MIB / 8) as usize;

/// Memory that must stay available while a memory test runs: `max(1 GiB, 5 % of RAM)`.
pub(crate) fn memory_reserve(total: u64) -> u64 {
    (total / 20).max(GIB)
}

fn refuse(msg: String) -> AppError {
    AppError::new(
        ErrorCode::InvalidInput,
        "not enough available memory to run the test safely",
    )
    .with_detail(msg)
}

/// Bandwidth buffer: `min(1 GiB, 25 % of available)`, rounded down to 64 MiB, at least
/// 256 MiB, and the reserve must remain available after allocating it.
pub(crate) fn bandwidth_buffer_size(available: u64, total: u64) -> Result<u64, AppError> {
    let size = (available / 4).min(BANDWIDTH_MAX_BUFFER) / (64 * MIB) * (64 * MIB);
    if size < MIN_BUFFER {
        return Err(refuse(format!(
            "25% of available memory ({} MiB) is below the {} MiB minimum",
            available / 4 / MIB,
            MIN_BUFFER / MIB
        )));
    }
    check_reserve(available, total, size)?;
    Ok(size)
}

/// Latency buffer: fixed 256 MiB (comparability), with the same safety rules.
pub(crate) fn latency_buffer_size(available: u64, total: u64) -> Result<u64, AppError> {
    if available / 4 < LATENCY_BUFFER {
        return Err(refuse(format!(
            "the {} MiB buffer exceeds 25% of available memory ({} MiB)",
            LATENCY_BUFFER / MIB,
            available / MIB
        )));
    }
    check_reserve(available, total, LATENCY_BUFFER)?;
    Ok(LATENCY_BUFFER)
}

fn check_reserve(available: u64, total: u64, size: u64) -> Result<(), AppError> {
    let reserve = memory_reserve(total);
    if available.saturating_sub(size) < reserve {
        return Err(refuse(format!(
            "available {} MiB - buffer {} MiB would fall below the {} MiB reserve",
            available / MIB,
            size / MIB,
            reserve / MIB
        )));
    }
    Ok(())
}

fn current_memory() -> Result<(u64, u64), AppError> {
    let m = platform::process::memory_status()?;
    Ok((m.available_bytes, m.total_bytes))
}

/// Allocates and touches `len` u64s (zero-filled) in 64 MiB steps, checking cancellation.
fn alloc_touched(
    ctx: &RunCtx,
    len: usize,
    phase: &str,
    f0: f64,
    f1: f64,
) -> Result<Vec<u64>, RunError> {
    let mut v: Vec<u64> = Vec::new();
    v.try_reserve_exact(len).map_err(|e| {
        AppError::new(ErrorCode::Internal, "could not allocate the test buffer")
            .with_detail(e.to_string())
    })?;
    let step = (64 * MIB / 8) as usize;
    while v.len() < len {
        ctx.check()?;
        let n = step.min(len - v.len());
        v.extend(std::iter::repeat_n(0u64, n));
        ctx.set_phase(phase, f0 + (f1 - f0) * v.len() as f64 / len as f64);
    }
    Ok(v)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Read,
    Write,
    Copy,
}

fn sum(s: &[u64]) -> u64 {
    s.iter().fold(0u64, |a, &x| a.wrapping_add(x))
}

/// One worker's share of one pass. Returns `None` if aborted mid-pass.
fn pass_chunk(
    op: Op,
    a: &mut [u64],
    b: &mut [u64],
    pattern: u64,
    abort: &dyn Fn() -> bool,
) -> Option<u64> {
    let mut acc = 0u64;
    match op {
        Op::Read => {
            for s in a.chunks(SUB_CHUNK).chain(b.chunks(SUB_CHUNK)) {
                if abort() {
                    return None;
                }
                acc = acc.wrapping_add(sum(black_box(s)));
            }
        }
        Op::Write => {
            for s in a.chunks_mut(SUB_CHUNK).chain(b.chunks_mut(SUB_CHUNK)) {
                if abort() {
                    return None;
                }
                s.fill(pattern);
                black_box(&s[0]);
            }
        }
        Op::Copy => {
            for (src, dst) in a.chunks(SUB_CHUNK).zip(b.chunks_mut(SUB_CHUNK)) {
                if abort() {
                    return None;
                }
                dst.copy_from_slice(src);
                black_box(&dst[0]);
            }
        }
    }
    Some(acc)
}

/// A reusable barrier that can be broken: once broken every current and future `wait`
/// returns `false` immediately. A failed spawn or an early exit can therefore never leave
/// workers blocked forever.
struct BreakableBarrier {
    parties: usize,
    state: Mutex<(usize, u64, bool)>, // (arrived, generation, broken)
    cv: Condvar,
}

impl BreakableBarrier {
    fn new(parties: usize) -> Self {
        Self {
            parties,
            state: Mutex::new((0, 0, false)),
            cv: Condvar::new(),
        }
    }

    /// `true` when all parties arrived, `false` if the barrier is (or gets) broken.
    fn wait(&self) -> bool {
        let mut g = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if g.2 {
            return false;
        }
        let generation = g.1;
        g.0 += 1;
        if g.0 == self.parties {
            g.0 = 0;
            g.1 = g.1.wrapping_add(1);
            self.cv.notify_all();
            return true;
        }
        while g.1 == generation && !g.2 {
            g = self.cv.wait(g).unwrap_or_else(PoisonError::into_inner);
        }
        g.1 != generation
    }

    fn break_all(&self) {
        let mut g = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        g.2 = true;
        self.cv.notify_all();
    }
}

fn op_id(op: Op) -> &'static str {
    match op {
        Op::Read => "read_bandwidth",
        Op::Write => "write_bandwidth",
        Op::Copy => "copy_bandwidth",
    }
}

fn op_label(op: Op, lang: Language) -> &'static str {
    match op {
        Op::Read => lang.pick("Banda in lettura", "Read bandwidth"),
        Op::Write => lang.pick("Banda in scrittura", "Write bandwidth"),
        Op::Copy => lang.pick("Banda in copia", "Copy bandwidth"),
    }
}

fn op_phase(op: Op, lang: Language) -> &'static str {
    match op {
        Op::Read => lang.pick("Lettura", "Reading"),
        Op::Write => lang.pick("Scrittura", "Writing"),
        Op::Copy => lang.pick("Copia", "Copying"),
    }
}

fn phase_check_memory(lang: Language) -> &'static str {
    lang.pick("Controllo memoria disponibile", "Checking available memory")
}

fn phase_buffer(lang: Language) -> &'static str {
    lang.pick("Preparazione buffer", "Preparing the buffer")
}

pub(crate) fn run_bandwidth(ctx: &RunCtx, threads: u32) -> Result<Outcome, RunError> {
    let lang = ctx.lang;
    ctx.set_phase(phase_check_memory(lang), 0.0);
    let (available, total) = current_memory()?;
    let size = bandwidth_buffer_size(available, total)?;
    let threads = threads.max(1) as usize;
    let len = (size / 8) as usize;
    let mut buf = alloc_touched(ctx, len, phase_buffer(lang), 0.0, 0.1)?;

    // Halves are the copy source/destination; each worker owns one chunk of each half.
    let half = len / 2;
    let per = half.div_ceil(threads);
    let ops = [Op::Read, Op::Write, Op::Copy];
    let total_passes = ops.len() * PASSES;
    let failed = AtomicBool::new(false);
    let mut rates: Vec<Vec<f64>> = vec![Vec::new(); ops.len()];
    let pattern_of =
        |pass: usize| 0x0123_4567_89AB_CDEF_u64.rotate_left(pass as u32 % 64) ^ pass as u64;

    let checksum = {
        let (ha, hb) = buf.split_at_mut(half);
        let pairs: Vec<(&mut [u64], &mut [u64])> =
            ha.chunks_mut(per).zip(hb.chunks_mut(per)).collect();
        let barrier = BreakableBarrier::new(pairs.len() + 1);
        // Per-worker (start, end) of the current pass. Timing is taken by the workers
        // themselves: the coordinating thread may not get a CPU while all workers run, so
        // its own clock readings would be late and inflate the rates.
        let spans: Vec<Mutex<Option<(Instant, Instant)>>> =
            (0..pairs.len()).map(|_| Mutex::new(None)).collect();
        std::thread::scope(|s| -> Result<u64, RunError> {
            let mut handles = Vec::with_capacity(pairs.len());
            for (i, (a, b)) in pairs.into_iter().enumerate() {
                let (barrier, failed, span) = (&barrier, &failed, &spans[i]);
                let spawned = ctx.spawn_worker(s, &format!("bench-mem-{i}"), move || {
                    let mut acc = 0u64;
                    let abort = || ctx.is_cancelled() || failed.load(Ordering::Relaxed);
                    for pass in 0..total_passes {
                        if !barrier.wait() {
                            break;
                        }
                        let op = ops[pass / PASSES];
                        let started = Instant::now();
                        let r = catch_unwind(AssertUnwindSafe(|| {
                            pass_chunk(op, a, b, pattern_of(pass), &abort)
                        }));
                        *span.lock().unwrap_or_else(PoisonError::into_inner) =
                            Some((started, Instant::now()));
                        match r {
                            Ok(Some(v)) => acc = acc.wrapping_add(v),
                            Ok(None) => {}
                            Err(_) => failed.store(true, Ordering::Release),
                        }
                        if !barrier.wait() {
                            break;
                        }
                    }
                    acc
                });
                match spawned {
                    Ok(h) => handles.push(h),
                    Err(e) => {
                        barrier.break_all();
                        join_all(handles);
                        return Err(e.into());
                    }
                }
            }

            for pass in 0..total_passes {
                if ctx.is_cancelled() || failed.load(Ordering::Acquire) {
                    break;
                }
                let op_idx = pass / PASSES;
                ctx.set_phase(
                    op_phase(ops[op_idx], lang),
                    0.1 + 0.9 * pass as f64 / total_passes as f64,
                );
                for sp in &spans {
                    *sp.lock().unwrap_or_else(PoisonError::into_inner) = None;
                }
                barrier.wait();
                barrier.wait();
                if ctx.is_cancelled() || failed.load(Ordering::Acquire) {
                    break;
                }
                let mut first: Option<Instant> = None;
                let mut last: Option<Instant> = None;
                for sp in &spans {
                    let Some((a, b)) = *sp.lock().unwrap_or_else(PoisonError::into_inner) else {
                        continue;
                    };
                    first = Some(first.map_or(a, |f| f.min(a)));
                    last = Some(last.map_or(b, |l| l.max(b)));
                }
                let (Some(first), Some(last)) = (first, last) else {
                    continue;
                };
                let dt = (last - first).as_secs_f64();
                // Bytes per pass: read = all, write = all, copy = half read + half written.
                let rate = size as f64 / dt;
                rates[op_idx].push(rate);
                ctx.set_interim(vec![metric(
                    op_id(ops[op_idx]),
                    op_label(ops[op_idx], lang),
                    rate,
                    Unit::BytesPerSecond,
                    true,
                )]);
            }
            barrier.break_all();
            Ok(join_all(handles).into_iter().fold(0u64, u64::wrapping_add))
        })?
    };
    black_box(checksum);
    if failed.load(Ordering::Acquire) {
        return Err(RunError::Failed(AppError::internal(
            "a memory worker panicked",
        )));
    }
    ctx.check()?;

    // Verify the last copy: destination equals source at sampled positions.
    let (ha, hb) = buf.split_at(half);
    let step = (half / 4096).max(1);
    if (0..half).step_by(step).any(|i| ha[i] != hb[i]) {
        return Err(RunError::Failed(AppError::internal(
            "copy verification failed: destination differs from source",
        )));
    }
    drop(buf);

    let mut out = Outcome::default();
    for (i, op) in ops.iter().enumerate() {
        let v = &rates[i];
        let (Some(med), Some(best)) = (stats::median(v), stats::max(v)) else {
            return Err(RunError::Failed(AppError::internal(
                "no completed memory pass",
            )));
        };
        out.metrics.push(metric(
            op_id(*op),
            op_label(*op, lang),
            med,
            Unit::BytesPerSecond,
            true,
        ));
        out.metrics.push(metric(
            &format!("{}_best", op_id(*op)),
            &best_label(*op, lang),
            best,
            Unit::BytesPerSecond,
            true,
        ));
    }
    out.metrics.push(metric(
        "buffer_size",
        lang.pick("Dimensione buffer", "Buffer size"),
        size as f64,
        Unit::Bytes,
        true,
    ));
    out.warnings.push(
        lang.pick(
            "Il test misura la banda, non verifica l'assenza di errori della RAM.",
            "The test measures bandwidth; it doesn't check the RAM for errors.",
        )
        .into(),
    );
    Ok(out)
}

fn best_label(op: Op, lang: Language) -> String {
    let label = op_label(op, lang);
    lang.pick(format!("{label} (migliore)"), format!("{label} (best)"))
}

/// Builds a single random cycle over `lines` cache lines (Sattolo's algorithm) inside
/// `buf`: `buf[i * 8]` holds the element index of the next line. Deterministic seed.
fn build_chain(ctx: &RunCtx, buf: &mut [u64], lines: usize) -> Result<(), RunError> {
    for i in 0..lines {
        buf[i * 8] = (i * 8) as u64;
    }
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    for i in (1..lines).rev() {
        if i % CHASE_INNER == 0 {
            ctx.check()?;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let j = (x % i as u64) as usize;
        buf.swap(i * 8, j * 8);
    }
    Ok(())
}

/// Chases the chain for up to `CHASE_BUDGET` after one untimed warm-up batch; returns the
/// median ns per access over batches of `CHASE_BATCH` dependent loads.
fn chase(ctx: &RunCtx, buf: &[u64]) -> Result<f64, RunError> {
    let mut idx = 0usize;
    for _ in 0..CHASE_BATCH / CHASE_INNER {
        ctx.check()?;
        for _ in 0..CHASE_INNER {
            idx = buf[idx] as usize;
        }
    }
    let mut per_access = Vec::new();
    let started = Instant::now();
    while per_access.len() < CHASE_MAX_BATCHES
        && (per_access.len() < 3 || started.elapsed() < CHASE_BUDGET)
    {
        let t0 = Instant::now();
        for _ in 0..CHASE_BATCH / CHASE_INNER {
            ctx.check()?;
            for _ in 0..CHASE_INNER {
                idx = buf[idx] as usize;
            }
        }
        per_access.push(t0.elapsed().as_nanos() as f64 / CHASE_BATCH as f64);
    }
    black_box(idx);
    stats::median(&per_access)
        .ok_or_else(|| RunError::Failed(AppError::internal("no pointer-chase batch completed")))
}

pub(crate) fn run_latency(ctx: &RunCtx) -> Result<Outcome, RunError> {
    let lang = ctx.lang;
    ctx.set_phase(phase_check_memory(lang), 0.0);
    let (available, total) = current_memory()?;
    let size = latency_buffer_size(available, total)?;

    ctx.set_phase(
        lang.pick("Riferimento in cache (32 KiB)", "Cache reference (32 KiB)"),
        0.05,
    );
    let ref_lines = (LATENCY_CACHE_REF / 64) as usize;
    let mut small = vec![0u64; ref_lines * 8];
    build_chain(ctx, &mut small, ref_lines)?;
    let cache_ns = chase(ctx, &small)?;
    drop(small);

    let len = (size / 8) as usize;
    let mut buf = alloc_touched(ctx, len, phase_buffer(lang), 0.1, 0.3)?;
    ctx.set_phase(
        lang.pick("Costruzione catena casuale", "Building the random chain"),
        0.3,
    );
    build_chain(ctx, &mut buf, len / 8)?;
    ctx.set_phase(lang.pick("Misura latenza", "Measuring latency"), 0.5);
    let ram_ns = chase(ctx, &buf)?;
    drop(buf);

    let mut out = Outcome::default();
    out.metrics.push(metric(
        "latency",
        lang.pick("Latenza RAM (indicativa)", "RAM latency (indicative)"),
        ram_ns,
        Unit::Nanoseconds,
        false,
    ));
    out.metrics.push(metric(
        "latency_cache_32k",
        lang.pick(
            "Latenza di riferimento in cache (32 KiB)",
            "Cache reference latency (32 KiB)",
        ),
        cache_ns,
        Unit::Nanoseconds,
        false,
    ));
    out.warnings.push(latency_warning(lang));
    Ok(out)
}

/// The note on what the latency figure includes. Windows: 4 KiB pages (the buffer is an
/// ordinary allocation). macOS: the VM page size the OS reports (16 KiB on Apple Silicon).
#[cfg(not(target_os = "macos"))]
fn latency_warning(lang: Language) -> String {
    lang.pick(
        "Latenza indicativa: accessi dipendenti casuali su 256 MiB con pagine da 4 KiB \
         (include gli effetti della TLB). Non verifica l'assenza di errori della RAM.",
        "Indicative latency: random dependent accesses over 256 MiB with 4 KiB pages \
         (includes TLB effects). It doesn't check the RAM for errors.",
    )
    .into()
}

#[cfg(target_os = "macos")]
fn latency_warning(lang: Language) -> String {
    // SAFETY: sysconf only reads a system constant.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let pages = match page {
        p if p >= 1024 => lang.pick(
            format!(" con pagine da {} KiB", p / 1024),
            format!(" with {} KiB pages", p / 1024),
        ),
        _ => String::new(),
    };
    lang.pick(
        format!(
            "Latenza indicativa: accessi dipendenti casuali su 256 MiB{pages} (include gli \
             effetti della TLB). Non verifica l'assenza di errori della RAM."
        ),
        format!(
            "Indicative latency: random dependent accesses over 256 MiB{pages} (includes TLB \
             effects). It doesn't check the RAM for errors."
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_warning_names_the_page_size() {
        let en = latency_warning(Language::English);
        let it = latency_warning(Language::Italian);
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            // Apple Silicon: 16 KiB VM pages.
            assert!(
                en.contains("over 256 MiB with 16 KiB pages (includes TLB"),
                "{en}"
            );
            assert!(
                it.contains("su 256 MiB con pagine da 16 KiB (include"),
                "{it}"
            );
        } else if cfg!(not(target_os = "macos")) {
            assert!(en.contains("with 4 KiB pages"), "{en}");
            assert!(it.contains("con pagine da 4 KiB"), "{it}");
        }
    }
    use crate::runtime::Hooks;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn bandwidth_sizing() {
        let total = 32 * GIB;
        // Plenty of memory: capped at 1 GiB.
        assert_eq!(bandwidth_buffer_size(20 * GIB, total).expect("ok"), GIB);
        // 25 % of 2 GiB = 512 MiB, but 2 GiB - 512 MiB < reserve (1.6 GiB) → refuse.
        assert!(bandwidth_buffer_size(2 * GIB, total).is_err());
        // 25 % of 3 GiB = 768 MiB; 3 GiB - 768 MiB = 2.25 GiB ≥ 1.6 GiB.
        assert_eq!(
            bandwidth_buffer_size(3 * GIB, total).expect("ok"),
            768 * MIB
        );
        // Below the 256 MiB minimum.
        let e = bandwidth_buffer_size(900 * MIB, 4 * GIB).expect_err("too small");
        assert_eq!(e.code, ErrorCode::InvalidInput);
        // Rounded down to 64 MiB.
        let s = bandwidth_buffer_size(2 * GIB + 100 * MIB, 8 * GIB).expect("ok");
        assert_eq!(s % (64 * MIB), 0);
        assert!((MIN_BUFFER..=BANDWIDTH_MAX_BUFFER).contains(&s));
    }

    #[test]
    fn latency_sizing_and_reserve() {
        assert_eq!(memory_reserve(8 * GIB), GIB);
        assert_eq!(memory_reserve(40 * GIB), 2 * GIB);
        assert_eq!(
            latency_buffer_size(8 * GIB, 32 * GIB).expect("ok"),
            LATENCY_BUFFER
        );
        assert!(latency_buffer_size(900 * MIB, 32 * GIB).is_err());
        // 25 % fits, reserve does not: 1.5 GiB - 256 MiB < 1.6 GiB.
        assert!(latency_buffer_size(GIB + 512 * MIB, 32 * GIB).is_err());
    }

    #[test]
    fn chain_is_a_single_cycle() {
        let ctx = RunCtx::new(
            Hooks::default(),
            Arc::new(AtomicUsize::new(0)),
            Language::Italian,
        );
        let lines = 1000;
        let mut buf = vec![0u64; lines * 8];
        build_chain(&ctx, &mut buf, lines).expect("chain");
        let mut seen = vec![false; lines];
        let mut idx = 0usize;
        for _ in 0..lines {
            let line = idx / 8;
            assert!(!seen[line], "line {line} visited twice");
            seen[line] = true;
            idx = buf[idx] as usize;
        }
        assert_eq!(idx, 0, "returns to the start after visiting every line");
        assert!(seen.iter().all(|s| *s));
    }

    #[test]
    fn pass_kernels_compute_and_stop_on_abort() {
        let go = || false;
        let mut a = vec![1u64; 1000];
        let mut b = vec![2u64; 1000];
        assert_eq!(pass_chunk(Op::Read, &mut a, &mut b, 0, &go), Some(3000));
        pass_chunk(Op::Write, &mut a, &mut b, 7, &go);
        assert!(a.iter().chain(b.iter()).all(|v| *v == 7));
        a.fill(5);
        pass_chunk(Op::Copy, &mut a, &mut b, 0, &go);
        assert_eq!(a, b);
        assert_eq!(pass_chunk(Op::Read, &mut a, &mut b, 0, &|| true), None);
    }

    #[test]
    fn labels_in_both_languages() {
        assert_eq!(
            best_label(Op::Copy, Language::Italian),
            "Banda in copia (migliore)"
        );
        assert_eq!(
            best_label(Op::Copy, Language::English),
            "Copy bandwidth (best)"
        );
        assert_eq!(op_phase(Op::Write, Language::English), "Writing");
        assert_eq!(
            phase_check_memory(Language::English),
            "Checking available memory"
        );
    }

    #[test]
    fn breakable_barrier_releases_waiters() {
        let b = BreakableBarrier::new(3);
        std::thread::scope(|s| {
            let h1 = s.spawn(|| b.wait());
            let h2 = s.spawn(|| b.wait());
            std::thread::sleep(Duration::from_millis(50));
            b.break_all();
            assert!(!h1.join().expect("join"));
            assert!(!h2.join().expect("join"));
        });
        assert!(!b.wait(), "stays broken");
        // Normal use: all parties arrive, repeatedly.
        let b = BreakableBarrier::new(2);
        std::thread::scope(|s| {
            let h = s.spawn(|| (0..100).all(|_| b.wait()));
            assert!((0..100).all(|_| b.wait()));
            assert!(h.join().expect("join"));
        });
    }
}
