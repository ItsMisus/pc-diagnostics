//! Read-only device and driver facts (phase 4).
//!
//! - [`enumerate_present_devices`]: SetupAPI + CfgMgr32 (fast, local, no COM).
//! - [`signed_drivers`]: WMI `Win32_PnPSignedDriver` (slow: seconds). Own thread, bounded.
//! - [`search_driver_updates`]: Windows Update Agent search for driver updates (network,
//!   may take minutes). Own MTA thread, bounded. Search only: nothing is downloaded or
//!   installed.
//!
//! Nothing in this module changes device or driver state. On non-Windows platforms every
//! entry point returns `NotSupported`.
//!
//! The pure helpers (multi-sz parsing, date conversions) are unit-tested on every platform.

use std::time::Duration;

use models::AppError;

/// Configuration Manager status of one device node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevNodeStatus {
    /// `DN_STARTED` is set.
    pub started: bool,
    /// `CM_PROB_*` when `DN_HAS_PROBLEM` is set.
    pub problem_code: Option<u32>,
}

/// One present device as reported by SetupAPI / CfgMgr32. Strings are trimmed; empty
/// values are `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDevice {
    pub instance_id: String,
    pub friendly_name: Option<String>,
    pub description: Option<String>,
    pub manufacturer: Option<String>,
    /// Setup class name (`Display`, `Net`, …). Not localized.
    pub class_name: Option<String>,
    /// Setup class GUID, lower case with braces.
    pub class_guid: Option<String>,
    /// Most specific first, as Windows orders them.
    pub hardware_ids: Vec<String>,
    /// `Err` carries the CfgMgr32 failure (rare for present devices).
    pub status: Result<DevNodeStatus, String>,
    pub driver_version: Option<String>,
    /// ISO 8601 date (`DEVPKEY_Device_DriverDate`).
    pub driver_date: Option<String>,
    pub driver_provider: Option<String>,
    /// INF name in the driver store (`oem12.inf`, `display.inf`).
    pub driver_inf: Option<String>,
}

/// One row of `Win32_PnPSignedDriver`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedDriverRow {
    /// Equal to the device instance id.
    pub device_id: String,
    pub is_signed: Option<bool>,
    pub signer: Option<String>,
    pub driver_version: Option<String>,
    /// ISO 8601 date.
    pub driver_date: Option<String>,
    pub provider: Option<String>,
    pub inf_name: Option<String>,
}

/// One driver update offered by Windows Update (`IWindowsDriverUpdate`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverUpdateOffer {
    pub title: String,
    pub hardware_id: String,
    pub provider: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub class: Option<String>,
    /// `DriverVerDate` as ISO 8601 date.
    pub date: Option<String>,
}

/// Every present device (`DIGCF_PRESENT | DIGCF_ALLCLASSES`), in SetupAPI order.
pub fn enumerate_present_devices() -> Result<Vec<RawDevice>, AppError> {
    #[cfg(windows)]
    {
        crate::windows::devices::enumerate_present()
    }
    #[cfg(not(windows))]
    {
        Err(not_supported("device enumeration"))
    }
}

/// Signature data for every driver-bound device, from WMI. Runs on its own thread and gives
/// up after `timeout` (the worker is then left to finish on its own).
pub fn signed_drivers(timeout: Duration) -> Result<Vec<SignedDriverRow>, AppError> {
    #[cfg(windows)]
    {
        crate::windows::pnp_wmi::signed_drivers(timeout)
    }
    #[cfg(not(windows))]
    {
        let _ = timeout;
        Err(not_supported("Win32_PnPSignedDriver"))
    }
}

/// Searches Windows Update for applicable, not-installed driver updates. Network access;
/// blocking; may take minutes. Never downloads or installs.
pub fn search_driver_updates(timeout: Duration) -> Result<Vec<DriverUpdateOffer>, AppError> {
    #[cfg(windows)]
    {
        crate::windows::wua::search_driver_updates(timeout)
    }
    #[cfg(not(windows))]
    {
        let _ = timeout;
        Err(not_supported("Windows Update Agent"))
    }
}

#[cfg(not(windows))]
fn not_supported(what: &str) -> AppError {
    AppError::new(
        models::ErrorCode::NotSupported,
        format!("{what} is only available on Windows"),
    )
}

/// Splits a `REG_MULTI_SZ` / `DEVPROP_TYPE_STRING_LIST` buffer (NUL-separated, double-NUL
/// terminated). Stops at the first empty string; entries are trimmed; empty ones dropped.
pub fn parse_multi_sz(buf: &[u16]) -> Vec<String> {
    let mut out = Vec::new();
    for part in buf.split(|c| *c == 0) {
        if part.is_empty() {
            break;
        }
        let s = String::from_utf16_lossy(part);
        let s = s.trim();
        if !s.is_empty() {
            out.push(s.to_owned());
        }
    }
    out
}

/// UTF-16 up to the first NUL, trimmed; `None` when empty.
pub fn utf16_to_string(buf: &[u16]) -> Option<String> {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..end]);
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// Little-endian byte buffer (as returned by property APIs) to UTF-16 code units.
pub fn bytes_to_u16(buf: &[u8]) -> Vec<u16> {
    let (pairs, _odd) = buf.as_chunks::<2>();
    pairs.iter().map(|c| u16::from_le_bytes(*c)).collect()
}

/// Gregorian date from days since 1970-01-01 (H. Hinnant's `civil_from_days`).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (y, m as u32, d as u32)
}

fn iso_from_unix_days(days: i64) -> Option<String> {
    let (y, m, d) = civil_from_days(days);
    (1601..=9999)
        .contains(&y)
        .then(|| format!("{y:04}-{m:02}-{d:02}"))
}

/// Days between 1601-01-01 (FILETIME epoch) and 1970-01-01.
const FILETIME_EPOCH_DAYS: i64 = 134_774;
const FILETIME_TICKS_PER_DAY: u64 = 864_000_000_000;

/// FILETIME (100 ns ticks since 1601-01-01 UTC) to an ISO 8601 date. Driver dates are
/// stored as midnight UTC of the INF `DriverVer` date, so the UTC date is the declared one.
/// `0` (unset) gives `None`.
pub fn filetime_to_iso_date(ticks: u64) -> Option<String> {
    if ticks == 0 {
        return None;
    }
    let days = i64::try_from(ticks / FILETIME_TICKS_PER_DAY).ok()?;
    iso_from_unix_days(days - FILETIME_EPOCH_DAYS)
}

/// Days between 1899-12-30 (OLE automation `DATE` epoch) and 1970-01-01.
const OLE_EPOCH_DAYS: i64 = 25_569;

/// OLE automation `DATE` (days since 1899-12-30, fraction = time of day) to an ISO 8601
/// date. Non-positive or non-finite values (unset) give `None`.
pub fn ole_date_to_iso_date(value: f64) -> Option<String> {
    if !value.is_finite() || value < 1.0 || value > 3_000_000.0 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let days = value.floor() as i64;
    iso_from_unix_days(days - OLE_EPOCH_DAYS)
}

/// CIM `DATETIME` (`yyyymmddHHMMSS.mmmmmmsUUU`) to an ISO 8601 date (date part only).
pub fn cim_datetime_to_iso_date(value: &str) -> Option<String> {
    let digits = value.get(..8)?;
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (y, m, d) = (&digits[..4], &digits[4..6], &digits[6..8]);
    let (mn, dn): (u32, u32) = (m.parse().ok()?, d.parse().ok()?);
    if !(1..=12).contains(&mn) || !(1..=31).contains(&dn) || y == "0000" {
        return None;
    }
    Some(format!("{y}-{m}-{d}"))
}

/// Formats a GUID from its fields as `{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}` (lower case,
/// registry style). The all-zero GUID gives `None`.
pub fn format_guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Option<String> {
    if data1 == 0 && data2 == 0 && data3 == 0 && data4 == [0; 8] {
        return None;
    }
    Some(format!(
        "{{{data1:08x}-{data2:04x}-{data3:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
        data4[0], data4[1], data4[2], data4[3], data4[4], data4[5], data4[6], data4[7]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn multi_sz_splits_and_stops_at_double_nul() {
        let mut buf = w("PCI\\VEN_8086&DEV_1234\0PCI\\VEN_8086\0\0garbage\0");
        assert_eq!(
            parse_multi_sz(&buf),
            vec!["PCI\\VEN_8086&DEV_1234", "PCI\\VEN_8086"]
        );
        buf.clear();
        assert!(parse_multi_sz(&buf).is_empty());
        assert!(parse_multi_sz(&[0, 0]).is_empty());
        // Missing terminator and surrounding blanks.
        assert_eq!(parse_multi_sz(&w(" A \0B")), vec!["A", "B"]);
    }

    #[test]
    fn utf16_string_stops_at_nul_and_trims() {
        assert_eq!(utf16_to_string(&w(" Intel \0junk")), Some("Intel".into()));
        assert_eq!(utf16_to_string(&w("  \0")), None);
        assert_eq!(utf16_to_string(&[]), None);
    }

    #[test]
    fn bytes_to_u16_is_little_endian_and_drops_odd_byte() {
        assert_eq!(
            bytes_to_u16(&[0x41, 0x00, 0x42, 0x00, 0x01]),
            vec![0x41, 0x42]
        );
    }

    #[test]
    fn filetime_dates() {
        assert_eq!(filetime_to_iso_date(0), None);
        // 1970-01-01T00:00:00Z.
        assert_eq!(
            filetime_to_iso_date(116_444_736_000_000_000).as_deref(),
            Some("1970-01-01")
        );
        // 2006-06-21 (the classic inbox driver date): Unix 1_150_848_000 s.
        assert_eq!(
            filetime_to_iso_date((1_150_848_000 + 11_644_473_600) * 10_000_000).as_deref(),
            Some("2006-06-21")
        );
        // 2024-02-29 23:59:59.9999999 stays on the 29th (leap day).
        let leap_midnight = (1_709_164_800 + 11_644_473_600) * 10_000_000_u64;
        assert_eq!(
            filetime_to_iso_date(leap_midnight).as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(
            filetime_to_iso_date(leap_midnight + FILETIME_TICKS_PER_DAY - 1).as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(filetime_to_iso_date(1).as_deref(), Some("1601-01-01"));
    }

    #[test]
    fn ole_dates() {
        assert_eq!(ole_date_to_iso_date(0.0), None);
        assert_eq!(ole_date_to_iso_date(f64::NAN), None);
        assert_eq!(
            ole_date_to_iso_date(25_569.0).as_deref(),
            Some("1970-01-01")
        );
        assert_eq!(
            ole_date_to_iso_date(45_351.75).as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(ole_date_to_iso_date(2.0).as_deref(), Some("1900-01-01"));
    }

    #[test]
    fn cim_dates() {
        assert_eq!(
            cim_datetime_to_iso_date("20230615000000.******+000").as_deref(),
            Some("2023-06-15")
        );
        assert_eq!(cim_datetime_to_iso_date("2023"), None);
        assert_eq!(cim_datetime_to_iso_date("2023AB15000000"), None);
        assert_eq!(cim_datetime_to_iso_date("20231315000000"), None);
    }

    #[test]
    fn guid_format() {
        assert_eq!(
            format_guid(
                0x4d36_e968,
                0xe325,
                0x11ce,
                [0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18]
            )
            .as_deref(),
            Some("{4d36e968-e325-11ce-bfc1-08002be10318}")
        );
        assert_eq!(format_guid(0, 0, 0, [0; 8]), None);
    }
}
