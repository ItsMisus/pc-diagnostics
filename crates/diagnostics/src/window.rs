//! Statistics over the live monitoring window.

use models::monitor::LiveSample;
use models::{Metric, Source};

/// Minimum observed window for every "sustained" rule.
pub const SUSTAINED_MIN_WINDOW_S: u32 = 60;
/// Below this process analysis window, the processes coverage is noted as short.
pub const SHORT_WINDOW_S: u32 = 30;
/// A condition is sustained when it holds in at least this share of the available samples.
pub const SUSTAINED_SHARE_PERCENT: usize = 60;
/// A metric is usable only if it is available in at least this share of the samples.
pub const MIN_AVAILABLE_SHARE_PERCENT: usize = 50;

/// Seconds covered by the samples: span between first and last sample plus one median
/// interval (each sample measures the interval preceding it). One sample → 0 (unknown).
pub fn observed_window_s(live: &[LiveSample]) -> u32 {
    if live.len() < 2 {
        return 0;
    }
    let mut deltas: Vec<u64> = live
        .windows(2)
        .map(|w| w[1].at.saturating_sub(w[0].at))
        .collect();
    deltas.sort_unstable();
    let median = deltas[deltas.len() / 2];
    let span = live[live.len() - 1].at.saturating_sub(live[0].at);
    let total_ms = span.saturating_add(median);
    u32::try_from((total_ms + 500) / 1000).unwrap_or(u32::MAX)
}

/// Value of an available metric.
pub fn val(m: &Metric<f64>) -> Option<f64> {
    m.value().copied().filter(|v| v.is_finite())
}

pub fn src<T>(m: &Metric<T>) -> Option<Source> {
    match m {
        Metric::Available { source, .. } => Some(*source),
        Metric::Unavailable { .. } => None,
    }
}

/// How often a per-sample condition held across the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sustained {
    pub total: usize,
    pub available: usize,
    pub hits: usize,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
}

impl Sustained {
    /// `extract` returns the per-sample value (None = unavailable); `hit` tests it.
    pub fn compute(
        live: &[LiveSample],
        extract: impl Fn(&LiveSample) -> Option<f64>,
        hit: impl Fn(f64) -> bool,
    ) -> Self {
        let mut s = Self {
            total: live.len(),
            available: 0,
            hits: 0,
            mean: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        };
        let mut sum = 0.0;
        for v in live.iter().filter_map(extract).filter(|v| v.is_finite()) {
            s.available += 1;
            sum += v;
            s.min = s.min.min(v);
            s.max = s.max.max(v);
            if hit(v) {
                s.hits += 1;
            }
        }
        if s.available > 0 {
            s.mean = sum / s.available as f64;
        } else {
            s.min = 0.0;
            s.max = 0.0;
        }
        s
    }

    /// The metric is available in enough samples to be judged.
    pub fn usable(&self) -> bool {
        self.available > 0 && self.available * 100 >= self.total * MIN_AVAILABLE_SHARE_PERCENT
    }

    /// Share of available samples where the condition held, 0–100.
    pub fn hit_percent(&self) -> f64 {
        if self.available == 0 {
            0.0
        } else {
            self.hits as f64 * 100.0 / self.available as f64
        }
    }

    /// Integer comparison so the 60 % boundary is exact.
    pub fn is_sustained(&self, window_s: u32) -> bool {
        window_s >= SUSTAINED_MIN_WINDOW_S
            && self.usable()
            && self.hits * 100 >= self.available * SUSTAINED_SHARE_PERCENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::live;

    #[test]
    fn window_includes_one_interval() {
        assert_eq!(observed_window_s(&live(0, |_, _| {})), 0);
        assert_eq!(observed_window_s(&live(1, |_, _| {})), 0);
        assert_eq!(observed_window_s(&live(60, |_, _| {})), 60);
        assert_eq!(observed_window_s(&live(2, |_, _| {})), 2);
    }

    #[test]
    fn sustained_boundary_is_exactly_60_percent() {
        let samples = live(60, |_, _| {});
        let values: Vec<f64> = (0..60).map(|i| if i < 36 { 90.0 } else { 10.0 }).collect();
        let s = Sustained::compute(
            &samples,
            |x| Some(values[(x.seq - 1) as usize]),
            |v| v >= 85.0,
        );
        assert_eq!(s.hits, 36);
        assert!(s.is_sustained(60));
        assert!(!s.is_sustained(59));
        let s = Sustained::compute(
            &samples,
            |x| Some(if x.seq <= 35 { 90.0 } else { 10.0 }),
            |v| v >= 85.0,
        );
        assert!(!s.is_sustained(60));
    }

    #[test]
    fn mostly_unavailable_metric_is_not_usable() {
        let samples = live(10, |_, _| {});
        let s = Sustained::compute(&samples, |x| (x.seq <= 4).then_some(99.0), |v| v >= 85.0);
        assert!(!s.usable());
        let s = Sustained::compute(&samples, |x| (x.seq <= 5).then_some(99.0), |v| v >= 85.0);
        assert!(s.usable());
    }
}
