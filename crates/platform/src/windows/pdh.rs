//! Minimal safe wrapper over the Performance Data Helper (PDH) API.
//!
//! Counters are added with `PdhAddEnglishCounterW`, so paths are always English and work on
//! localized Windows (e.g. Italian, where the display name of `\Memory` differs).

use ::windows::Win32::System::Performance::{
    PDH_CALC_NEGATIVE_DENOMINATOR, PDH_CALC_NEGATIVE_TIMEBASE, PDH_CALC_NEGATIVE_VALUE,
    PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_NO_COUNTER, PDH_CSTATUS_NO_OBJECT, PDH_CSTATUS_VALID_DATA,
    PDH_FMT, PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER,
    PDH_HQUERY, PDH_MORE_DATA, PDH_RAW_COUNTER_ITEM_W, PdhAddEnglishCounterW, PdhCloseQuery,
    PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhGetFormattedCounterValue,
    PdhGetRawCounterArrayW, PdhOpenQueryW,
};
use ::windows::core::PCWSTR;
use models::{AppError, ErrorCode, Unavailability};

/// `PDH_FMT_NOCAP100`: do not cap percentages at 100 (we clamp ourselves, after summing).
const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
const FMT: PDH_FMT = PDH_FMT(PDH_FMT_DOUBLE.0 | PDH_FMT_NOCAP100);

pub fn pdh_error(op: &str, status: u32) -> AppError {
    AppError::new(ErrorCode::Platform, format!("{op} failed"))
        .with_detail(format!("PDH status 0x{status:08X}"))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Owned PDH query. Closing the query also frees every counter added to it; callers keep
/// each `Counter` next to its `Query` (same struct) and never use it after the query drops.
pub struct Query {
    handle: PDH_HQUERY,
}

impl Query {
    pub fn open() -> Result<Self, AppError> {
        let mut handle = PDH_HQUERY::default();
        // SAFETY: null data source = real-time; `handle` is a valid out pointer.
        let status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut handle) };
        if status != 0 {
            return Err(pdh_error("PdhOpenQueryW", status));
        }
        Ok(Self { handle })
    }

    /// Adds an English counter path. Wildcard instances (`(*)`) are expanded by PDH at
    /// every collection, so processes that appear later are included.
    /// A missing object/counter (e.g. no `GPU Engine` object on this OS) maps to
    /// `NotSupported`; anything else is `Failed`.
    pub fn add(&self, path: &str) -> Result<Counter, Unavailability> {
        let w = wide(path);
        let mut counter = PDH_HCOUNTER::default();
        // SAFETY: `w` is NUL-terminated and outlives the call; `counter` is a valid out pointer;
        // `self.handle` is an open query.
        let status =
            unsafe { PdhAddEnglishCounterW(self.handle, PCWSTR(w.as_ptr()), 0, &mut counter) };
        if status == PDH_CSTATUS_NO_OBJECT || status == PDH_CSTATUS_NO_COUNTER {
            return Err(Unavailability::NotSupported);
        }
        if status != 0 {
            return Err(Unavailability::Failed(pdh_error(
                &format!("PdhAddEnglishCounterW({path})"),
                status,
            )));
        }
        Ok(Counter { handle: counter })
    }

    pub fn collect(&self) -> Result<(), AppError> {
        // SAFETY: `self.handle` is an open query.
        let status = unsafe { PdhCollectQueryData(self.handle) };
        if status != 0 {
            return Err(pdh_error("PdhCollectQueryData", status));
        }
        Ok(())
    }
}

impl Drop for Query {
    fn drop(&mut self) {
        // SAFETY: the handle came from PdhOpenQueryW and is closed exactly once.
        let _ = unsafe { PdhCloseQuery(self.handle) };
    }
}

/// Counter handle, valid while its `Query` lives.
#[derive(Clone, Copy)]
pub struct Counter {
    handle: PDH_HCOUNTER,
}

fn is_calc_warning(status: u32) -> bool {
    matches!(
        status,
        PDH_CALC_NEGATIVE_DENOMINATOR | PDH_CALC_NEGATIVE_TIMEBASE | PDH_CALC_NEGATIVE_VALUE
    )
}

fn status_ok(cstatus: u32) -> bool {
    cstatus == PDH_CSTATUS_VALID_DATA || cstatus == PDH_CSTATUS_NEW_DATA
}

impl Counter {
    /// Formatted value of a single-instance counter.
    pub fn value(&self) -> Result<f64, AppError> {
        let mut v = PDH_FMT_COUNTERVALUE::default();
        // SAFETY: valid counter handle; `v` is a valid out pointer; type out-param is optional.
        let status = unsafe { PdhGetFormattedCounterValue(self.handle, FMT, None, &mut v) };
        if status != 0 {
            return Err(pdh_error("PdhGetFormattedCounterValue", status));
        }
        if !status_ok(v.CStatus) {
            return Err(pdh_error("PDH counter status", v.CStatus));
        }
        // SAFETY: PDH_FMT_DOUBLE was requested, so the union holds `doubleValue`.
        Ok(unsafe { v.Anonymous.doubleValue })
    }

    /// Formatted values of a wildcard counter, as `(instance name, value)`. `value` is
    /// `None` for instances without valid data this round (e.g. an instance that has only
    /// one raw sample yet); callers decide what to do with those.
    pub fn array(&self) -> Result<Vec<(String, Option<f64>)>, AppError> {
        let (buf, count) = read_array::<PDH_FMT_COUNTERVALUE_ITEM_W>(
            "PdhGetFormattedCounterArrayW",
            |size, count, ptr| {
                // SAFETY: `size` is the byte capacity of `ptr` (0 with no buffer, which asks
                // PDH for the required size); PDH writes at most that many bytes.
                unsafe { PdhGetFormattedCounterArrayW(self.handle, FMT, size, count, ptr) }
            },
        )?;
        Ok(buf[..count]
            .iter()
            .filter_map(|item| {
                // SAFETY: names point into `buf`, written by PDH, NUL-terminated.
                let name = unsafe { item_name(item.szName) }?;
                let value = if status_ok(item.FmtValue.CStatus) {
                    // SAFETY: PDH_FMT_DOUBLE was requested.
                    Some(unsafe { item.FmtValue.Anonymous.doubleValue })
                } else {
                    None
                };
                Some((name, value))
            })
            .collect())
    }

    /// Raw values of a wildcard counter (for computing rates ourselves). Instances whose
    /// raw status is invalid are skipped.
    pub fn raw_array(&self) -> Result<Vec<(String, RawValue)>, AppError> {
        let (buf, count) =
            read_array::<PDH_RAW_COUNTER_ITEM_W>("PdhGetRawCounterArrayW", |size, count, ptr| {
                // SAFETY: as in `array`.
                unsafe { PdhGetRawCounterArrayW(self.handle, size, count, ptr) }
            })?;
        Ok(buf[..count]
            .iter()
            .filter_map(|item| {
                if !status_ok(item.RawValue.CStatus) {
                    return None;
                }
                // SAFETY: as in `array`.
                let name = unsafe { item_name(item.szName) }?;
                let ts = item.RawValue.TimeStamp;
                Some((
                    name,
                    RawValue {
                        first: item.RawValue.FirstValue,
                        second: item.RawValue.SecondValue,
                        timestamp: (u64::from(ts.dwHighDateTime) << 32)
                            | u64::from(ts.dwLowDateTime),
                    },
                ))
            })
            .collect())
    }
}

/// One raw counter reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawValue {
    pub first: i64,
    pub second: i64,
    /// FILETIME of the collection, 100 ns units.
    pub timestamp: u64,
}

/// # Safety
/// `p` must be null or point to a NUL-terminated UTF-16 string.
unsafe fn item_name(p: ::windows::core::PWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: guaranteed by the caller.
    unsafe { p.to_string() }.ok()
}

/// Two-call PDH array protocol: ask for the size, allocate, read. Instances can appear
/// between the calls, so it retries a few times. Returns the buffer and the item count.
fn read_array<T: Default + Clone>(
    op: &str,
    mut fetch: impl FnMut(&mut u32, &mut u32, Option<*mut T>) -> u32,
) -> Result<(Vec<T>, usize), AppError> {
    let item_size = std::mem::size_of::<T>();
    let mut buf: Vec<T> = Vec::new();
    for _ in 0..4 {
        let mut size_bytes = u32::try_from(buf.len() * item_size).unwrap_or(u32::MAX);
        let mut count = 0u32;
        let ptr = (!buf.is_empty()).then_some(buf.as_mut_ptr());
        let status = fetch(&mut size_bytes, &mut count, ptr);
        // Calculation warnings are per instance; the array itself is still produced.
        let filled = status == 0 || is_calc_warning(status);
        if status == PDH_MORE_DATA || (filled && buf.is_empty() && size_bytes > 0) {
            // Items are followed by their name strings: round up and add headroom.
            let n = (size_bytes as usize).div_ceil(item_size) + 16;
            buf = vec![T::default(); n];
            continue;
        }
        if !filled {
            return Err(pdh_error(op, status));
        }
        let count = (count as usize).min(buf.len());
        return Ok((buf, count));
    }
    Err(AppError::new(
        ErrorCode::Platform,
        format!("{op}: instance list kept growing"),
    ))
}
