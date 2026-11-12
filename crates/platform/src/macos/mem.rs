//! Virtual-memory counters from the Mach host (`host_statistics64(HOST_VM_INFO64)`), no
//! privileges needed.
//!
//! `MemorySample.pages_per_sec` is documented as hard page faults resolved from disk
//! (`\Memory\Pages/sec` on Windows). The macOS counter with that meaning is
//! `vm_statistics64.pageins`: pages the pager read from backing store (mapped files,
//! executables) to resolve a fault. The rate is its delta over the elapsed wall time.
//! The unit is the VM page: 16 KiB on Apple Silicon, 4 KiB on Intel Macs and Windows, so
//! the same byte volume gives a quarter of the page count on Apple Silicon.
//! Not included:
//! - `pageouts`: pages written out; they do not resolve a fault (Windows' counter also
//!   adds pages written, so the macOS value is the read half of it).
//! - `swapins`: compressor segments read back from the swap file are counted per
//!   compressed segment page, not per faulting page, so adding them would mix units.
//!
//! macOS has no commit charge and no commit limit (the compressor and dynamic swap files
//! replace a fixed page file), so `commit_used_bytes` / `commit_limit_bytes` stay
//! `NotSupported`. The "memory is short" signal on macOS is the kernel's pressure level
//! (`kern.memorystatus_vm_pressure_level`, what Activity Monitor graphs) and, as
//! context, the swap in use (`vm.swapusage`). Both sysctls are readable without root.

use std::sync::OnceLock;
use std::time::Instant;

use models::{AppError, ErrorCode};

// `libc::mach_host_self` is deprecated in favour of the `mach2` crate; the symbol itself
// is a stable libSystem export.
unsafe extern "C" {
    fn mach_host_self() -> libc::mach_port_t;
}

/// The host name port, obtained once per process. `mach_host_self` adds a send-right
/// reference on every call, so caching it avoids leaking one reference per tick.
fn host_port() -> libc::mach_port_t {
    static PORT: OnceLock<libc::mach_port_t> = OnceLock::new();
    // SAFETY: no arguments, no preconditions; returns the host port name (or 0).
    *PORT.get_or_init(|| unsafe { mach_host_self() })
}

/// One `HOST_VM_INFO64` reading.
pub fn vm_statistics() -> Result<libc::vm_statistics64, AppError> {
    let port = host_port();
    if port == 0 {
        return Err(AppError::new(
            ErrorCode::Platform,
            "mach_host_self returned no port",
        ));
    }
    // SAFETY: `vm_statistics64` is plain integers, so all-zero is a valid value.
    let mut stat: libc::vm_statistics64 = unsafe { std::mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: `stat` is a writable buffer of `count` integers (the count is derived from
    // the struct size), which is exactly what `HOST_VM_INFO64` expects.
    let r = unsafe {
        libc::host_statistics64(
            port,
            libc::HOST_VM_INFO64,
            (&raw mut stat).cast::<libc::integer_t>(),
            &raw mut count,
        )
    };
    if r == libc::KERN_SUCCESS {
        Ok(stat)
    } else {
        Err(
            AppError::new(ErrorCode::Platform, "host_statistics64 failed")
                .with_detail(format!("kern_return {r}")),
        )
    }
}

/// Reads a fixed-size sysctl value by name. The returned length must match `T` exactly,
/// so a kernel that changed the layout is reported as a failure, not misread.
fn sysctl_value<T: Copy>(name: &std::ffi::CStr, zero: T) -> Result<T, AppError> {
    let mut v = zero;
    let mut len = std::mem::size_of::<T>();
    // SAFETY: `name` is NUL-terminated; `v`/`len` describe a writable buffer of
    // `size_of::<T>()` bytes; no new value is written (null, 0).
    let r = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&raw mut v).cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    let label = name.to_string_lossy();
    if r != 0 {
        let err = std::io::Error::last_os_error();
        return Err(
            AppError::new(ErrorCode::Platform, format!("sysctl {label} failed"))
                .with_detail(err.to_string()),
        );
    }
    if len != std::mem::size_of::<T>() {
        return Err(AppError::new(
            ErrorCode::Platform,
            format!("sysctl {label}: unexpected size"),
        )
        .with_detail(format!("{len} bytes")));
    }
    Ok(v)
}

/// Swap `(used, total)` in bytes from `vm.swapusage`. Total is 0 when no swap file exists
/// yet (macOS creates them on demand).
pub fn swap_usage() -> Result<(u64, u64), AppError> {
    // SAFETY: `xsw_usage` is plain integers and a bool-like u32, so all-zero is valid.
    let zero: libc::xsw_usage = unsafe { std::mem::zeroed() };
    let u = sysctl_value(c"vm.swapusage", zero)?;
    if u.xsu_used > u.xsu_total {
        return Err(
            AppError::new(ErrorCode::Platform, "vm.swapusage: used above total")
                .with_detail(format!("{} > {}", u.xsu_used, u.xsu_total)),
        );
    }
    Ok((u.xsu_used, u.xsu_total))
}

/// The kernel memory-pressure level (`kern.memorystatus_vm_pressure_level`).
pub fn pressure_level() -> Result<models::monitor::MemoryPressure, AppError> {
    let level: libc::c_int = sysctl_value(c"kern.memorystatus_vm_pressure_level", 0)?;
    models::monitor::MemoryPressure::from_kernel_level(level).ok_or_else(|| {
        AppError::new(ErrorCode::Platform, "unknown memory pressure level")
            .with_detail(level.to_string())
    })
}

/// Rate of a monotonically increasing counter between two readings. `None` when the
/// window is empty or the counter went backwards (it is never reset while running, so
/// that would mean a bad reading, not a real negative rate).
pub fn counter_rate(prev: u64, now: u64, elapsed_s: f64) -> Option<f64> {
    if elapsed_s.is_nan() || elapsed_s <= 0.0 || now < prev {
        return None;
    }
    #[allow(clippy::cast_precision_loss)] // deltas per tick are far below 2^53
    Some((now - prev) as f64 / elapsed_s)
}

/// `pageins` per second between consecutive calls.
pub struct PageIns {
    prev: Option<(u64, Instant)>,
}

impl PageIns {
    /// Takes the baseline reading.
    pub fn new() -> Self {
        let mut p = Self { prev: None };
        let _ = p.rate();
        p
    }

    /// Pages read from backing store per second since the previous call.
    pub fn rate(&mut self) -> Result<f64, AppError> {
        let now = Instant::now();
        let stat = vm_statistics()?;
        let prev = self.prev.replace((stat.pageins, now));
        let Some((prev_count, prev_at)) = prev else {
            let mut e = AppError::new(
                ErrorCode::NotFound,
                "No completed measurement window yet (needs two readings)",
            );
            e.retriable = true;
            return Err(e);
        };
        let elapsed = now.duration_since(prev_at).as_secs_f64();
        counter_rate(prev_count, stat.pageins, elapsed).ok_or_else(|| {
            AppError::new(ErrorCode::Platform, "pageins counter went backwards")
                .with_detail(format!("{prev_count} -> {}", stat.pageins))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_reject_empty_windows_and_backwards_counters() {
        assert_eq!(counter_rate(10, 30, 2.0), Some(10.0));
        assert_eq!(counter_rate(10, 10, 1.0), Some(0.0));
        assert_eq!(counter_rate(10, 9, 1.0), None);
        assert_eq!(counter_rate(10, 20, 0.0), None);
        assert_eq!(counter_rate(10, 20, f64::NAN), None);
    }

    #[test]
    fn host_vm_statistics_are_plausible() {
        let s = vm_statistics().expect("host_statistics64");
        assert!(s.free_count > 0 || s.inactive_count > 0);
        assert!(s.pageins > 0, "every booted Mac has paged in executables");
        let mut p = PageIns::new();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let r = p.rate().expect("rate after baseline");
        assert!(r.is_finite() && r >= 0.0);
    }

    #[test]
    fn host_swap_and_pressure_are_readable_without_root() {
        let (used, total) = swap_usage().expect("vm.swapusage");
        assert!(used <= total);
        // Swap files are allocated in whole 1 MiB units or more.
        assert_eq!(total % (1024 * 1024), 0, "{total}");
        pressure_level().expect("kern.memorystatus_vm_pressure_level");
    }
}
