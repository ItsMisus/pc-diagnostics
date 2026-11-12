//! CPU frequency facts on macOS.
//!
//! - Intel Macs: `sysctl hw.cpufrequency_max` (Hz), the rated frequency, the same meaning
//!   as Windows' `MaxMhz`.
//! - Apple Silicon: there is no base clock and no sysctl. The rated maximum is the top
//!   entry of the performance-cluster DVFS table that the power manager publishes in the
//!   device tree (`IODeviceTree` node `pmgr`, property `voltage-states5-sram`: pairs of
//!   little-endian `u32` frequency + voltage; M1–M3 store Hz, M4 and later kHz, told
//!   apart by magnitude). The efficiency cluster's table (`voltage-states1-sram`) must
//!   top out lower, which checks that the table really is the performance cluster's;
//!   otherwise the value is not reported.
//!
//! Current frequency and an OS frequency limit have no unprivileged public source on
//! Apple Silicon (`powermetrics` needs root; the per-cluster residency counters are in
//! the private IOReport framework), so those stay unavailable.

use models::metric::{Metric, Source, Unavailability, Unit};

use super::iokit;

/// Frequency entries (MHz) of a DVFS table: `[freq u32 LE, voltage u32 LE]` pairs.
/// Zero frequencies (unused slots) are skipped.
pub fn dvfs_mhz(table: &[u8]) -> Vec<f64> {
    let (entries, _partial) = table.as_chunks::<8>();
    let raw: Vec<u32> = entries
        .iter()
        .map(|[a, b, c, d, ..]| u32::from_le_bytes([*a, *b, *c, *d]))
        .filter(|f| *f > 0)
        .collect();
    // Hz tables start above 100 MHz = 1e8; kHz tables stay below 1e7 (10 GHz).
    let divisor = if raw.iter().any(|f| *f >= 100_000_000) {
        1e6
    } else {
        1e3
    };
    raw.into_iter().map(|f| f64::from(f) / divisor).collect()
}

fn table_max(table: &[u8]) -> Option<f64> {
    dvfs_mhz(table).into_iter().reduce(f64::max)
}

/// Performance-cluster maximum from the two `pmgr` tables, if they are consistent.
pub fn perf_cluster_max_mhz(perf: &[u8], efficiency: Option<&[u8]>) -> Option<f64> {
    let p = table_max(perf)?;
    let plausible = (500.0..=10_000.0).contains(&p);
    let above_e = efficiency.and_then(table_max).is_none_or(|e| p > e);
    (plausible && above_e).then_some(p)
}

fn sysctl_u64(name: &std::ffi::CStr) -> Option<u64> {
    let mut v: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: `name` is NUL-terminated; `v`/`len` describe a writable 8-byte buffer.
    let r = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&raw mut v).cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (r == 0 && len == std::mem::size_of::<u64>()).then_some(v)
}

/// Rated maximum CPU frequency (see the module docs). Reads the registry once; cache it.
pub fn max_mhz() -> Metric<f64> {
    if let Some(hz) = sysctl_u64(c"hw.cpufrequency_max").filter(|v| *v > 0) {
        #[allow(clippy::cast_precision_loss)] // Hz values are far below 2^53
        return Metric::available(hz as f64 / 1e6, Unit::Megahertz, Source::Mach);
    }
    let pmgr = match iokit::services_of_class(c"AppleARMIODevice") {
        Ok(list) => list
            .into_iter()
            .find(|d| d.name().is_some_and(|n| n == "pmgr")),
        Err(e) => return Metric::unavailable(Unavailability::Failed(e)),
    };
    let Some(pmgr) = pmgr else {
        return Metric::unavailable(Unavailability::NotSupported);
    };
    let Some(perf) = pmgr.data("voltage-states5-sram") else {
        return Metric::unavailable(Unavailability::NotSupported);
    };
    let eff = pmgr.data("voltage-states1-sram");
    match perf_cluster_max_mhz(&perf, eff.as_deref()) {
        Some(mhz) => Metric::available(mhz, Unit::Megahertz, Source::IoKit),
        None => Metric::unavailable(Unavailability::NotSupported),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(freqs: &[u32]) -> Vec<u8> {
        freqs
            .iter()
            .flat_map(|f| {
                let mut e = f.to_le_bytes().to_vec();
                e.extend_from_slice(&800u32.to_le_bytes());
                e
            })
            .collect()
    }

    #[test]
    fn decodes_khz_and_hz_tables() {
        assert_eq!(
            dvfs_mhz(&table(&[1_308_000, 4_464_000, 4_416_000])),
            vec![1308.0, 4464.0, 4416.0]
        );
        assert_eq!(
            dvfs_mhz(&table(&[600_000_000, 3_228_000_000])),
            vec![600.0, 3228.0]
        );
        assert!(dvfs_mhz(&[1, 2, 3]).is_empty(), "partial entry ignored");
    }

    #[test]
    fn perf_table_must_top_the_efficiency_table() {
        let p = table(&[1_308_000, 4_428_000, 4_416_000, 4_464_000]);
        let e = table(&[972_000, 3_048_000]);
        assert_eq!(perf_cluster_max_mhz(&p, Some(&e)), Some(4464.0));
        assert_eq!(perf_cluster_max_mhz(&e, Some(&p)), None);
        assert_eq!(perf_cluster_max_mhz(&p, None), Some(4464.0));
        assert_eq!(perf_cluster_max_mhz(&table(&[0, 0]), None), None);
    }

    #[test]
    fn host_max_frequency_is_plausible() {
        let m = max_mhz();
        if cfg!(target_arch = "aarch64") {
            let v = *m.value().expect("Apple Silicon pmgr DVFS table");
            assert!((2000.0..=6000.0).contains(&v), "{v} MHz");
        }
    }
}
