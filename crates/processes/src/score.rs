//! Pure rate and Impact Score math (methodology: docs/impact-score.md).
//!
//! Everything is monotone, bounded to 0–100 and free of OS calls, so it is unit-tested
//! on every platform.

use models::process::{ImpactWeights, Pressure};

/// Logical I/O rate (read + write) that maps to I/O pressure 100. Fixed so scores stay
/// comparable across machines and over time (see docs/impact-score.md).
pub const IO_REFERENCE_BYTES_PER_SEC: f64 = 100.0 * 1024.0 * 1024.0;

/// Available memory is floored at this fraction of total RAM before dividing, so RAM
/// pressure stays finite and smooth when the machine is almost out of memory.
pub const RAM_AVAILABLE_FLOOR_FRACTION: f64 = 0.02;

/// 100 ns units per second.
const TICKS_PER_SEC: f64 = 10_000_000.0;

fn clamp100(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

/// Why a rate could not be computed this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateGap {
    /// No earlier reading for this process instance.
    NoBaseline,
    /// The cumulative counter went backwards: re-baselined, no value this tick.
    CounterReset,
    /// Zero or negative wall-clock window (should not happen with a monotonic clock).
    EmptyWindow,
}

/// Δcounter / Δwall in counter units per second.
pub fn rate_per_sec(prev: u64, cur: u64, wall_secs: f64) -> Result<f64, RateGap> {
    if !(wall_secs.is_finite() && wall_secs > 0.0) {
        return Err(RateGap::EmptyWindow);
    }
    let delta = cur.checked_sub(prev).ok_or(RateGap::CounterReset)?;
    #[allow(clippy::cast_precision_loss)] // counters far below 2^52 in any real window
    Ok(delta as f64 / wall_secs)
}

/// Share of the whole machine, 0–100: Δ(kernel+user) / Δwall / logical processors.
/// A process saturating every logical processor is 100; one busy thread on a 16-thread
/// CPU is 6.25. Clamped (timer quantization can overshoot slightly).
pub fn cpu_percent(
    prev_100ns: u64,
    cur_100ns: u64,
    wall_secs: f64,
    logical_processors: u32,
) -> Result<f64, RateGap> {
    if logical_processors == 0 {
        return Err(RateGap::EmptyWindow);
    }
    let cpu_secs_per_sec = rate_per_sec(prev_100ns, cur_100ns, wall_secs)? / TICKS_PER_SEC;
    Ok(clamp100(
        cpu_secs_per_sec * 100.0 / f64::from(logical_processors),
    ))
}

/// CPU pressure = machine share, unscaled (it is already 0–100 and linear in work done).
pub fn cpu_pressure(cpu_percent: Option<f64>) -> f64 {
    cpu_percent.map_or(0.0, clamp100)
}

/// RAM pressure, 0–100: the working set as a percentage of the memory that is currently
/// available, `100 · min(1, ws / max(available, 2 % · total))`.
///
/// Equivalently `share_of_RAM × 1 / available_fraction`: the share of physical memory the
/// process holds, amplified as the system runs out of memory. A 4 GiB process on a 64 GiB
/// machine with 58 GiB free scores ~7; the same process with 5 % available scores 100.
/// Non-decreasing in `ws`, non-increasing in `available`.
pub fn ram_pressure(working_set: Option<f64>, total: f64, available: f64) -> f64 {
    let Some(ws) = working_set.filter(|v| v.is_finite() && *v > 0.0) else {
        return 0.0;
    };
    if !(total.is_finite() && total > 0.0) {
        return 0.0;
    }
    let available = if available.is_finite() {
        available.clamp(0.0, total)
    } else {
        0.0
    };
    let headroom = available.max(RAM_AVAILABLE_FLOOR_FRACTION * total);
    clamp100(100.0 * ws / headroom)
}

/// I/O pressure, 0–100: `100 · min(1, (read + write) / reference)`. Unavailable sides
/// count as 0.
pub fn io_pressure(read_bps: Option<f64>, write_bps: Option<f64>, reference: f64) -> f64 {
    if !(reference.is_finite() && reference > 0.0) {
        return 0.0;
    }
    let part = |v: Option<f64>| v.filter(|x| x.is_finite() && *x > 0.0).unwrap_or(0.0);
    clamp100(100.0 * (part(read_bps) + part(write_bps)) / reference)
}

/// Weighted sum of the three terms (each already 0–100), clamped to 0–100.
pub fn impact(cpu: f64, ram: f64, io: f64, w: ImpactWeights) -> f64 {
    clamp100(w.cpu * clamp100(cpu) + w.ram * clamp100(ram) + w.io * clamp100(io))
}

pub fn pressure(cpu: f64, ram: f64, io: f64, w: ImpactWeights) -> Pressure {
    Pressure {
        cpu: clamp100(cpu),
        ram: clamp100(ram),
        io: clamp100(io),
        impact: impact(cpu, ram, io, w),
    }
}

/// Mean and maximum of `values`; `None` when empty.
pub fn mean_and_peak(values: impl IntoIterator<Item = f64>) -> Option<(f64, f64)> {
    let (mut sum, mut n, mut peak) = (0.0, 0u32, f64::NEG_INFINITY);
    for v in values {
        sum += v;
        n += 1;
        peak = peak.max(v);
    }
    (n > 0).then(|| (sum / f64::from(n), peak))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const W: ImpactWeights = ImpactWeights::DEFAULT;

    #[test]
    fn cpu_is_normalized_by_logical_processors() {
        // 2 s of CPU time (20 M ticks) in 1 s of wall time on 8 logical CPUs = 25 %.
        let v = cpu_percent(0, 20_000_000, 1.0, 8).expect("rate");
        assert!((v - 25.0).abs() < 1e-9, "{v}");
        // Same work on 16 threads = 12.5 %; on 1 = clamp to 100.
        assert!((cpu_percent(0, 20_000_000, 1.0, 16).expect("rate") - 12.5).abs() < 1e-9);
        assert_eq!(cpu_percent(0, 20_000_000, 1.0, 1), Ok(100.0));
        // One saturated thread over 2 s on 32 logical CPUs.
        let v = cpu_percent(1_000, 1_000 + 20_000_000, 2.0, 32).expect("rate");
        assert!((v - 3.125).abs() < 1e-9, "{v}");
        assert_eq!(cpu_percent(5, 5, 2.0, 8), Ok(0.0));
    }

    #[test]
    fn counter_reset_and_empty_window_yield_no_value() {
        assert_eq!(cpu_percent(10, 5, 1.0, 8), Err(RateGap::CounterReset));
        assert_eq!(rate_per_sec(10, 5, 1.0), Err(RateGap::CounterReset));
        assert_eq!(rate_per_sec(0, 5, 0.0), Err(RateGap::EmptyWindow));
        assert_eq!(rate_per_sec(0, 5, f64::NAN), Err(RateGap::EmptyWindow));
        assert_eq!(cpu_percent(0, 5, 1.0, 0), Err(RateGap::EmptyWindow));
        assert_eq!(rate_per_sec(1_000, 3_000, 2.0), Ok(1_000.0));
    }

    #[test]
    fn cpu_pressure_is_identity_on_share() {
        assert_eq!(cpu_pressure(Some(37.5)), 37.5);
        assert_eq!(cpu_pressure(None), 0.0);
        assert_eq!(cpu_pressure(Some(250.0)), 100.0);
        assert_eq!(cpu_pressure(Some(f64::NAN)), 0.0);
    }

    #[test]
    fn ram_big_process_on_idle_machine_is_modest() {
        let total = 64.0 * GIB;
        let v = ram_pressure(Some(4.0 * GIB), total, 58.0 * GIB);
        assert!(v > 5.0 && v < 10.0, "{v}");
    }

    #[test]
    fn ram_same_process_under_memory_pressure_is_high() {
        let total = 64.0 * GIB;
        let v = ram_pressure(Some(4.0 * GIB), total, 0.05 * total);
        assert!(v >= 90.0, "{v}");
        // A small process stays low even when memory is tight.
        let small = ram_pressure(Some(0.1 * GIB), total, 0.05 * total);
        assert!(small < 5.0, "{small}");
    }

    #[test]
    fn ram_pressure_is_monotone_and_bounded() {
        let total = 16.0 * GIB;
        let mut last = -1.0;
        for i in 0..=64 {
            let ws = f64::from(i) * 0.25 * GIB;
            let v = ram_pressure(Some(ws), total, 4.0 * GIB);
            assert!((0.0..=100.0).contains(&v));
            assert!(v >= last, "non-decreasing in working set");
            last = v;
        }
        let mut last = 101.0;
        for i in 0..=64 {
            let avail = f64::from(i) * 0.25 * GIB;
            let v = ram_pressure(Some(2.0 * GIB), total, avail);
            assert!((0.0..=100.0).contains(&v));
            assert!(v <= last, "non-increasing in available memory");
            last = v;
        }
        // Degenerate inputs never produce NaN or out-of-range values.
        assert_eq!(ram_pressure(None, total, total), 0.0);
        assert_eq!(ram_pressure(Some(GIB), 0.0, 0.0), 0.0);
        assert_eq!(
            ram_pressure(Some(GIB), total, f64::NAN),
            100.0,
            "NaN = none free"
        );
        assert!(ram_pressure(Some(GIB), total, 2.0 * total) <= 100.0);
        // Available floor: zero available behaves like 2 % available.
        assert_eq!(
            ram_pressure(Some(0.1 * GIB), total, 0.0),
            ram_pressure(Some(0.1 * GIB), total, 0.02 * total)
        );
    }

    #[test]
    fn io_pressure_is_linear_to_reference_then_saturates() {
        let r = IO_REFERENCE_BYTES_PER_SEC;
        assert_eq!(io_pressure(Some(0.0), Some(0.0), r), 0.0);
        assert!((io_pressure(Some(r / 4.0), Some(r / 4.0), r) - 50.0).abs() < 1e-9);
        assert_eq!(io_pressure(Some(r), Some(r), r), 100.0);
        assert!((io_pressure(Some(r / 10.0), None, r) - 10.0).abs() < 1e-9);
        assert_eq!(io_pressure(None, None, r), 0.0);
        assert_eq!(io_pressure(Some(1e9), None, 0.0), 0.0);
        let mut last = -1.0;
        for i in 0..100 {
            let v = io_pressure(Some(f64::from(i) * 4e6), None, r);
            assert!((0.0..=100.0).contains(&v) && v >= last);
            last = v;
        }
    }

    #[test]
    fn impact_uses_default_weights() {
        assert!((impact(100.0, 0.0, 0.0, W) - 45.0).abs() < 1e-9);
        assert!((impact(0.0, 100.0, 0.0, W) - 25.0).abs() < 1e-9);
        assert!((impact(0.0, 0.0, 100.0, W) - 30.0).abs() < 1e-9);
        assert!((impact(100.0, 100.0, 100.0, W) - 100.0).abs() < 1e-9);
        assert!((impact(20.0, 40.0, 10.0, W) - (9.0 + 10.0 + 3.0)).abs() < 1e-9);
        let p = pressure(150.0, -5.0, 50.0, W);
        assert_eq!((p.cpu, p.ram, p.io), (100.0, 0.0, 50.0));
        assert!((p.impact - 60.0).abs() < 1e-9);
    }

    #[test]
    fn window_mean_and_peak() {
        assert_eq!(mean_and_peak([]), None);
        assert_eq!(mean_and_peak([10.0, 30.0, 20.0]), Some((20.0, 30.0)));
        assert_eq!(mean_and_peak([5.0]), Some((5.0, 5.0)));
    }
}
