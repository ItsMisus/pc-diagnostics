//! Deterministic number/byte/date formatting for user-facing text, per language.
//!
//! Italian: decimal comma, a space before `%` ("85,0 %"), ISO-like dates. English (US):
//! decimal point, no space before `%` ("85.0%"), "Nov 4, 2026, 1:21:05 PM" dates. Neither
//! uses a thousands separator.

use models::Unit;
use models::settings::Language;

/// Placeholder for a value that cannot be shown.
pub fn not_available(lang: Language) -> &'static str {
    lang.pick("n/d", "n/a")
}

/// Formats `v` with `decimals` digits and the language's decimal separator.
pub fn num(lang: Language, v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return not_available(lang).into();
    }
    let s = format!("{v:.decimals$}");
    match lang {
        Language::Italian => s.replace('.', ","),
        Language::English => s,
    }
}

/// "85,0 %" / "85.0%".
fn with_percent(lang: Language, n: String) -> String {
    lang.pick(format!("{n} %"), format!("{n}%"))
}

pub fn pct(lang: Language, v: f64) -> String {
    with_percent(lang, num(lang, v, 1))
}

pub fn bytes(lang: Language, b: f64) -> String {
    if !b.is_finite() || b < 0.0 {
        return not_available(lang).into();
    }
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} B", num(lang, v, 0))
    } else {
        format!("{} {}", num(lang, v, 1), UNITS[i])
    }
}

/// Percentage rounded to a whole number for plain-language text: "92 %" / "92%".
pub fn pct_plain(lang: Language, v: f64) -> String {
    with_percent(lang, num(lang, v.round(), 0))
}

/// Rounded percentage as spoken in a sentence. Italian adds the definite article, elided
/// where the number is spoken with a leading vowel: "il 92 %", "l'8 %", "l'80 %". English
/// has no article: same as `pct_plain` ("92%").
pub fn il_pct(lang: Language, v: f64) -> String {
    if lang == Language::English {
        return pct_plain(lang, v);
    }
    let n = v.round();
    let elided = n == 1.0 || n == 8.0 || n == 11.0 || (80.0..90.0).contains(&n);
    format!(
        "{}{}",
        if elided { "l'" } else { "il " },
        pct_plain(lang, v)
    )
}

/// Size for plain-language text, with the labels File Explorer uses (KB, MB, GB, TB, powers
/// of 1024), so the number matches what the user sees in Windows: "12,5 GB" / "12.5 GB".
pub fn bytes_plain(lang: Language, b: f64) -> String {
    bytes(lang, b)
        .replace("KiB", "KB")
        .replace("MiB", "MB")
        .replace("GiB", "GB")
        .replace("TiB", "TB")
}

/// Italian "a", "a e b", "a, b e c"; English "a", "a and b", "a, b, and c".
pub fn join_names<S: AsRef<str>>(lang: Language, names: &[S]) -> String {
    let names: Vec<&str> = names.iter().map(|s| s.as_ref()).collect();
    match (lang, names.as_slice()) {
        (_, []) => String::new(),
        (_, [one]) => (*one).to_string(),
        (Language::English, [a, b]) => format!("{a} and {b}"),
        (Language::English, [rest @ .., last]) => format!("{}, and {last}", rest.join(", ")),
        (Language::Italian, [rest @ .., last]) => format!("{} e {last}", rest.join(", ")),
    }
}

/// A truncated list followed by how many were left out: Italian "a, b e altri 2", English
/// "a, b, and 2 more" ("a and 2 more" for one name).
pub fn join_names_and_more<S: AsRef<str>>(lang: Language, names: &[S], more: usize) -> String {
    let listed = names
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>()
        .join(", ");
    match lang {
        Language::Italian => format!("{listed} e altri {more}"),
        Language::English if names.len() == 1 => format!("{listed} and {more} more"),
        Language::English => format!("{listed}, and {more} more"),
    }
}

pub fn bytes_u64(lang: Language, b: u64) -> String {
    bytes(lang, b as f64)
}

pub fn mhz(lang: Language, v: f64) -> String {
    format!("{} MHz", num(lang, v, 0))
}

/// "45 s", "2 min 5 s" (the same in both languages).
pub fn seconds(s: u32) -> String {
    if s < 120 {
        format!("{s} s")
    } else {
        format!("{} min {} s", s / 60, s % 60)
    }
}

/// SI-prefixed number with a trailing space: "21,8 T", "950 ".
fn si(lang: Language, v: f64) -> String {
    let abs = v.abs();
    let (scaled, prefix) = if abs >= 1e12 {
        (v / 1e12, "T")
    } else if abs >= 1e9 {
        (v / 1e9, "G")
    } else if abs >= 1e6 {
        (v / 1e6, "M")
    } else if abs >= 1e3 {
        (v / 1e3, "k")
    } else {
        (v, "")
    };
    let digits = if scaled.abs() < 10.0 {
        2
    } else if scaled.abs() < 100.0 {
        1
    } else {
        0
    };
    format!("{} {prefix}", num(lang, scaled, digits))
}

/// A benchmark/metric value with its unit.
pub fn value_with_unit(lang: Language, v: f64, unit: Unit) -> String {
    let n = |d| num(lang, v, d);
    match unit {
        Unit::None => n(2),
        Unit::Percent => pct(lang, v),
        Unit::Bytes => bytes(lang, v),
        Unit::BytesPerSecond => format!("{}/s", bytes(lang, v)),
        Unit::Megahertz => mhz(lang, v),
        Unit::Celsius => format!("{} °C", n(1)),
        Unit::Watts => format!("{} W", n(1)),
        Unit::Nanoseconds => format!("{} ns", n(1)),
        Unit::Microseconds => format!("{} µs", n(1)),
        Unit::Milliseconds => format!("{} ms", n(1)),
        Unit::Seconds => format!("{} s", n(1)),
        Unit::Count => n(0),
        Unit::OperationsPerSecond => format!("{} op/s", n(0)),
        Unit::FlopsPerSecond => format!("{}FLOP/s", si(lang, v)),
    }
}

/// UTC civil date-time from Unix milliseconds: (year, month, day, hour, minute, second).
pub fn civil_utc(ms: u64) -> (i64, u32, u32, u32, u32, u32) {
    let secs = (ms / 1000) as i64;
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    // Howard Hinnant's days_from_civil inverse.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        m,
        d,
        (sod / 3600) as u32,
        ((sod % 3600) / 60) as u32,
        (sod % 60) as u32,
    )
}

const MONTHS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Italian `2026-11-04 13:21:05`, English `Nov 4, 2026, 1:21:05 PM` (what `en-US`
/// `toLocaleString` shows in the UI).
fn civil_text(lang: Language, ms: u64) -> String {
    let (y, mo, d, h, mi, s) = civil_utc(ms);
    match lang {
        Language::Italian => format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}"),
        Language::English => {
            let month = MONTHS_EN[(mo as usize).saturating_sub(1).min(11)];
            let h12 = if h % 12 == 0 { 12 } else { h % 12 };
            let am_pm = if h < 12 { "AM" } else { "PM" };
            format!("{month} {d}, {y}, {h12}:{mi:02}:{s:02} {am_pm}")
        }
    }
}

/// `2026-11-04 01:21:05 UTC` / `Nov 4, 2026, 1:21:05 AM UTC`
pub fn datetime_utc(lang: Language, ms: u64) -> String {
    format!("{} UTC", civil_text(lang, ms))
}

/// Moves a UTC instant by an offset in minutes (for local civil time).
pub fn shift(ms: u64, utc_offset_minutes: Option<i32>) -> u64 {
    let delta = i64::from(utc_offset_minutes.unwrap_or(0)) * 60_000;
    u64::try_from(i64::try_from(ms).unwrap_or(i64::MAX).saturating_add(delta)).unwrap_or(0)
}

/// `2026-11-04 10:21:05 (UTC+01:00)` with an offset, else `2026-11-04 09:21:05 UTC`
/// (English: `Nov 4, 2026, 10:21:05 AM (UTC+01:00)`). The offset is printed, so the
/// instant stays unambiguous even across DST changes.
pub fn datetime_local(lang: Language, ms: u64, utc_offset_minutes: Option<i32>) -> String {
    let Some(off) = utc_offset_minutes else {
        return datetime_utc(lang, ms);
    };
    let sign = if off < 0 { '-' } else { '+' };
    let abs = off.unsigned_abs();
    format!(
        "{} (UTC{sign}{:02}:{:02})",
        civil_text(lang, shift(ms, Some(off))),
        abs / 60,
        abs % 60
    )
}

/// `2026-11-04T01:21:05Z`
pub fn iso_utc(ms: u64) -> String {
    let (y, mo, d, h, mi, s) = civil_utc(ms);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    const IT: Language = Language::Italian;
    const EN: Language = Language::English;

    #[test]
    fn local_times_carry_their_offset() {
        let ms = 1_793_791_265_000; // 2026-11-04T11:21:05Z
        assert_eq!(datetime_local(IT, ms, None), "2026-11-04 11:21:05 UTC");
        assert_eq!(
            datetime_local(IT, ms, Some(60)),
            "2026-11-04 12:21:05 (UTC+01:00)"
        );
        assert_eq!(
            datetime_local(IT, ms, Some(-330)),
            "2026-11-04 05:51:05 (UTC-05:30)"
        );
    }

    #[test]
    fn english_dates_use_us_style() {
        let ms = 1_793_791_265_000; // 2026-11-04T11:21:05Z
        assert_eq!(datetime_local(EN, ms, None), "Nov 4, 2026, 11:21:05 AM UTC");
        assert_eq!(
            datetime_local(EN, ms, Some(60)),
            "Nov 4, 2026, 12:21:05 PM (UTC+01:00)"
        );
        assert_eq!(
            datetime_local(EN, ms, Some(-330)),
            "Nov 4, 2026, 5:51:05 AM (UTC-05:30)"
        );
        assert_eq!(datetime_utc(EN, 0), "Jan 1, 1970, 12:00:00 AM UTC");
        assert_eq!(
            datetime_utc(EN, 1_793_755_260_000 + 13 * 3_600_000),
            "Nov 4, 2026, 2:21:00 PM UTC"
        );
    }

    #[test]
    fn flops_use_si_prefixes() {
        assert_eq!(
            value_with_unit(IT, 21_770_330_229_964.8, Unit::FlopsPerSecond),
            "21,8 TFLOP/s"
        );
        assert_eq!(
            value_with_unit(EN, 21_770_330_229_964.8, Unit::FlopsPerSecond),
            "21.8 TFLOP/s"
        );
        assert_eq!(
            value_with_unit(IT, 950.0, Unit::FlopsPerSecond),
            "950 FLOP/s"
        );
    }

    #[test]
    fn numbers_use_decimal_comma() {
        assert_eq!(num(IT, 85.0, 1), "85,0");
        assert_eq!(pct(IT, 9.96), "10,0 %");
        assert_eq!(num(IT, f64::NAN, 1), "n/d");
    }

    #[test]
    fn english_numbers_use_decimal_point() {
        assert_eq!(num(EN, 85.0, 1), "85.0");
        assert_eq!(pct(EN, 9.96), "10.0%");
        assert_eq!(num(EN, f64::NAN, 1), "n/a");
        assert_eq!(bytes(EN, 1536.0), "1.5 KiB");
        assert_eq!(mhz(EN, 4700.0), "4700 MHz");
        assert_eq!(value_with_unit(EN, 3.27, Unit::Milliseconds), "3.3 ms");
    }

    #[test]
    fn bytes_scale() {
        assert_eq!(bytes(IT, 512.0), "512 B");
        assert_eq!(bytes(IT, 1536.0), "1,5 KiB");
        assert_eq!(bytes_u64(IT, 16 << 30), "16,0 GiB");
    }

    #[test]
    fn plain_language_helpers() {
        assert_eq!(pct_plain(IT, 91.6), "92 %");
        assert_eq!(il_pct(IT, 91.6), "il 92 %");
        assert_eq!(il_pct(IT, 1.2), "l'1 %");
        assert_eq!(il_pct(IT, 8.0), "l'8 %");
        assert_eq!(il_pct(IT, 11.0), "l'11 %");
        assert_eq!(il_pct(IT, 84.0), "l'84 %");
        assert_eq!(il_pct(IT, 18.0), "il 18 %");
        assert_eq!(il_pct(IT, 100.0), "il 100 %");
        assert_eq!(bytes_plain(IT, 12.5 * 1024.0 * 1024.0 * 1024.0), "12,5 GB");
        assert_eq!(bytes_plain(IT, 1536.0), "1,5 KB");
        assert_eq!(join_names::<&str>(IT, &[]), "");
        assert_eq!(join_names(IT, &["a"]), "a");
        assert_eq!(join_names(IT, &["a", "b"]), "a e b");
        assert_eq!(join_names(IT, &["a", "b", "c"]), "a, b e c");
        assert_eq!(join_names_and_more(IT, &["a", "b"], 2), "a, b e altri 2");
    }

    #[test]
    fn english_plain_language_helpers() {
        assert_eq!(pct_plain(EN, 91.6), "92%");
        assert_eq!(il_pct(EN, 8.0), "8%");
        assert_eq!(bytes_plain(EN, 12.5 * 1024.0 * 1024.0 * 1024.0), "12.5 GB");
        assert_eq!(join_names::<&str>(EN, &[]), "");
        assert_eq!(join_names(EN, &["a"]), "a");
        assert_eq!(join_names(EN, &["a", "b"]), "a and b");
        assert_eq!(join_names(EN, &["a", "b", "c"]), "a, b, and c");
        assert_eq!(join_names_and_more(EN, &["a"], 3), "a and 3 more");
        assert_eq!(join_names_and_more(EN, &["a", "b"], 2), "a, b, and 2 more");
    }

    #[test]
    fn civil_dates() {
        assert_eq!(datetime_utc(IT, 0), "1970-01-01 00:00:00 UTC");
        // 2026-11-04 01:21:00 UTC
        assert_eq!(iso_utc(1_793_755_260_000), "2026-11-04T01:21:00Z");
        // Leap day.
        assert_eq!(iso_utc(951_782_400_000), "2000-02-29T00:00:00Z");
    }
}
