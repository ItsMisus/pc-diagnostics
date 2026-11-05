use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch. Fits in a JS `number` (< 2^53) for millennia.
pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
