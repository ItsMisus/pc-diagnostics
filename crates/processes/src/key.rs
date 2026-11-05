//! Stable process-instance key: `<pid>:<creation time as Unix ms>`.
//!
//! A PID alone is reused by Windows after a process exits; the creation time makes the
//! key unique per process instance.

use std::fmt;
use std::str::FromStr;

use models::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcessKey {
    pub pid: u32,
    pub created_unix_ms: u64,
}

impl fmt::Display for ProcessKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.pid, self.created_unix_ms)
    }
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit())
}

impl FromStr for ProcessKey {
    type Err = AppError;

    /// Strict: two runs of ASCII digits separated by one `:`; no sign, spaces or extras.
    fn from_str(s: &str) -> Result<Self, AppError> {
        let bad = || {
            AppError::invalid_input("process key must be `<pid>:<creation time in ms>`")
                .with_detail(format!(
                    "received {:?}",
                    s.chars().take(64).collect::<String>()
                ))
        };
        let (pid, ms) = s.split_once(':').ok_or_else(bad)?;
        if !digits(pid) || !digits(ms) {
            return Err(bad());
        }
        Ok(Self {
            pid: pid.parse().map_err(|_| bad())?,
            created_unix_ms: ms.parse().map_err(|_| bad())?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::ErrorCode;

    #[test]
    fn formats_and_parses_roundtrip() {
        let k = ProcessKey {
            pid: 1234,
            created_unix_ms: 1_762_000_000_123,
        };
        assert_eq!(k.to_string(), "1234:1762000000123");
        assert_eq!(
            "1234:1762000000123".parse::<ProcessKey>().expect("parse"),
            k
        );
        assert_eq!(
            "4:0".parse::<ProcessKey>().expect("parse"),
            ProcessKey {
                pid: 4,
                created_unix_ms: 0
            }
        );
    }

    #[test]
    fn rejects_malformed_keys() {
        for s in [
            "",
            "1234",
            ":5",
            "5:",
            "+5:6",
            "5:-6",
            " 5:6",
            "5:6 ",
            "5:6:7",
            "a:1",
            "4294967296:1", // pid overflows u32
            "1:99999999999999999999999",
        ] {
            let e = s.parse::<ProcessKey>().expect_err(s);
            assert_eq!(e.code, ErrorCode::InvalidInput, "{s}");
        }
    }

    #[test]
    fn orders_by_pid_then_creation() {
        let a = ProcessKey {
            pid: 9,
            created_unix_ms: 5,
        };
        let b = ProcessKey {
            pid: 10,
            created_unix_ms: 1,
        };
        assert!(a < b, "numeric, not lexicographic");
    }
}
