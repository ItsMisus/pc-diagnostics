//! Local time zone offset through `localtime_r` (`tm_gmtoff`), which applies the
//! system time zone rules including daylight saving time at the given instant.

/// Local UTC offset in minutes at `at_ms` (Unix epoch ms). `None` if the conversion
/// fails or the instant does not fit `time_t`.
pub fn utc_offset_minutes(at_ms: u64) -> Option<i32> {
    let secs: libc::time_t = (at_ms / 1000).try_into().ok()?;
    // SAFETY: `tm` is plain integers and a nullable `tm_zone` pointer; all-zero is valid.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers refer to live, properly aligned locals; `localtime_r` is the
    // thread-safe variant and writes only into `tm`.
    let r = unsafe { libc::localtime_r(&raw const secs, &raw mut tm) };
    if r.is_null() {
        return None;
    }
    i32::try_from(tm.tm_gmtoff / 60).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // The host's own zone: winter and summer offsets are both real offsets, and a DST
    // shift (where the zone has one) is at most two hours.
    #[test]
    fn host_offsets_follow_the_zone_rules() {
        let jan = utc_offset_minutes(1_768_478_400_000).expect("2026-01-15 offset");
        let jul = utc_offset_minutes(1_784_116_800_000).expect("2026-07-15 offset");
        for off in [jan, jul] {
            assert!((-14 * 60..=14 * 60).contains(&off), "{off}");
            assert_eq!(off % 15, 0, "{off}");
        }
        assert!((jan - jul).abs() <= 120, "{jan} vs {jul}");
    }
}
