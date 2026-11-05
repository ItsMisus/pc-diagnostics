//! Run ids: UUID-shaped (8-4-4-4-12 lower-case hex) strings, unique within the process
//! (a monotonic counter occupies the low bits) and very unlikely to collide across
//! processes (wall-clock nanoseconds + a randomly keyed SipHash). No extra dependency.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn new_run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let random = RandomState::new().hash_one((nanos, count, std::process::id()));
    let hi = nanos as u64; // truncation intended: low 64 bits of the timestamp
    let lo = (random & 0xFFFF_FFFF_0000_0000) | (count & 0xFFFF_FFFF);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        hi >> 32,
        (hi >> 16) & 0xFFFF,
        hi & 0xFFFF,
        lo >> 48,
        lo & 0xFFFF_FFFF_FFFF
    )
}

/// True for strings produced by [`new_run_id`] (shape check only).
pub(crate) fn is_run_id(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(c),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_and_well_formed() {
        let ids: HashSet<String> = (0..10_000).map(|_| new_run_id()).collect();
        assert_eq!(ids.len(), 10_000);
        assert!(ids.iter().all(|i| is_run_id(i)), "{:?}", ids.iter().next());
        assert!(!is_run_id("0000000-0000-0000-0000-000000000000"));
        assert!(!is_run_id("0000000G-0000-0000-0000-000000000000"));
        assert!(!is_run_id("00000000-0000-0000-0000-00000000000A"));
    }
}
