//! macOS providers. They sit behind the same return types from `models` as the Windows
//! providers in `windows/`; anything macOS does not expose stays an explained
//! `Unavailability`.

pub mod bench_env;
mod bench_iokit;
pub mod credentials;
pub mod locale;
pub mod process;
pub mod remediation;

pub mod cpu;
pub mod gpu;
pub mod iokit;
pub mod live;
pub mod mem;
pub mod nvme_smart;
pub mod sensors;
pub mod storage;
pub mod time;

/// True on physical Apple silicon. In a macOS virtual machine (e.g. the GitHub Actions
/// `macos-latest` runners) the hypervisor exposes a paravirtual GPU without an Apple vendor
/// id, a virtual disk without a model or SMART, and no temperature sensors, so host tests
/// that describe real hardware check those values only here.
#[cfg(test)]
pub(crate) fn physical_apple_silicon() -> bool {
    let mut vm: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>();
    // SAFETY: NUL-terminated name; `vm`/`len` describe a writable int; no new value.
    let r = unsafe {
        libc::sysctlbyname(
            c"kern.hv_vmm_present".as_ptr(),
            (&raw mut vm).cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    cfg!(target_arch = "aarch64") && !(r == 0 && vm == 1)
}
