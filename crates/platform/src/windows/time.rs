//! Local time zone offset for a given instant (DST-aware, from the Windows time zone).

use ::windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
use ::windows::Win32::System::Time::{
    FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
};

/// 100 ns intervals between 1601-01-01 and 1970-01-01.
const EPOCH_DIFF_100NS: u64 = 116_444_736_000_000_000;

fn to_filetime(v: u64) -> FILETIME {
    FILETIME {
        dwLowDateTime: (v & 0xFFFF_FFFF) as u32,
        dwHighDateTime: (v >> 32) as u32,
    }
}

fn from_filetime(f: FILETIME) -> u64 {
    (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime)
}

pub fn utc_offset_minutes(at_ms: u64) -> Option<i32> {
    let utc_ft = at_ms.checked_mul(10_000)?.checked_add(EPOCH_DIFF_100NS)?;
    let mut utc = SYSTEMTIME::default();
    // SAFETY: valid in/out pointers to plain structs.
    unsafe { FileTimeToSystemTime(&to_filetime(utc_ft), &mut utc) }.ok()?;
    let mut local = SYSTEMTIME::default();
    // SAFETY: `None` selects the active time zone; valid in/out pointers.
    unsafe { SystemTimeToTzSpecificLocalTime(None, &utc, &mut local) }.ok()?;
    let mut local_ft = FILETIME::default();
    // SAFETY: valid in/out pointers.
    unsafe { SystemTimeToFileTime(&local, &mut local_ft) }.ok()?;
    let diff = i128::from(from_filetime(local_ft)) - i128::from(utc_ft);
    i32::try_from(diff / 600_000_000).ok()
}
