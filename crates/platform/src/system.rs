use models::system::SystemInfo;
use sysinfo::System;

/// Reads static OS facts. Cheap (no WMI), safe to call on demand.
pub fn system_info() -> SystemInfo {
    SystemInfo {
        os_name: System::name().ok(),
        os_version: System::os_version().ok(),
        os_long_version: System::long_os_version().ok(),
        kernel_version: System::kernel_version().ok(),
        hostname: System::host_name().ok(),
        cpu_arch: System::cpu_arch(),
        build_arch: std::env::consts::ARCH.to_owned(),
        boot_time_unix_s: System::boot_time().ok(),
        uptime_s: System::uptime().ok(),
    }
}

/// Local UTC offset (minutes, DST-aware) at the instant `at_ms` (Unix epoch ms). `None`
/// when the OS cannot convert it (or on non-Windows builds).
pub fn utc_offset_minutes(at_ms: u64) -> Option<i32> {
    #[cfg(windows)]
    {
        crate::windows::time::utc_offset_minutes(at_ms)
    }
    #[cfg(not(windows))]
    {
        let _ = at_ms;
        None
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn utc_offset_is_whole_quarter_hours() {
        let now = models::time::now_unix_ms();
        let off = super::utc_offset_minutes(now).expect("offset");
        assert!((-14 * 60..=14 * 60).contains(&off));
        assert_eq!(off % 15, 0);
    }

    use super::*;

    // Runs against the real OS: asserts only invariants that hold on any supported host.
    #[test]
    fn reports_plausible_values_on_host() {
        let info = system_info();
        assert!(!info.build_arch.is_empty());
        assert!(!info.cpu_arch.is_empty());
        assert!(
            info.boot_time_unix_s.is_some_and(|t| t > 1_000_000_000),
            "boot time after 2001"
        );
        assert!(info.os_name.is_some());
    }
}
