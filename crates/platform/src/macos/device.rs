//! Raw reads behind `crate::form_factor` on macOS: the device-tree product name, the
//! model identifier and whether an internal battery is installed. All public registry /
//! sysctl reads, no privileges.

use std::ffi::CStr;

use models::device::FormFactorInfo;
use models::metric::{Metric, Source, Unavailability, Unit};

use super::iokit;

/// `IODeviceTree:/product` `product-name` (Apple silicon), e.g. "MacBook Air (13-inch,
/// M5)". Intel Macs have no such node: `None`.
pub fn product_name() -> Option<String> {
    let node = iokit::entry_from_path(c"IODeviceTree:/product")?;
    data_string(&node.data("product-name")?)
}

/// `hw.model`, e.g. "Mac17,3" (Apple silicon) or "MacBookPro16,1" (Intel).
pub fn model_identifier() -> Option<String> {
    sysctl_string(c"hw.model")
}

/// Whether an `IOPMPowerSource` reports `BatteryInstalled`. Macs without a battery may
/// publish no power source at all, which reads as "no internal battery".
pub fn internal_battery() -> Metric<bool> {
    match iokit::services_of_class(c"IOPMPowerSource") {
        Ok(list) => {
            let installed = list
                .iter()
                .any(|s| s.boolean("BatteryInstalled") == Some(true));
            Metric::available(installed, Unit::None, Source::IoKit)
        }
        Err(e) => Metric::unavailable(Unavailability::Failed(e)),
    }
}

pub fn form_factor() -> FormFactorInfo {
    crate::form_factor::mac_info(product_name(), model_identifier(), internal_battery())
}

/// Device-tree strings are NUL-terminated byte data.
fn data_string(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let s = String::from_utf8_lossy(&bytes[..end]).trim().to_owned();
    (!s.is_empty()).then_some(s)
}

fn sysctl_string(name: &CStr) -> Option<String> {
    let mut len: usize = 0;
    // SAFETY: size query: NUL-terminated name, null buffer, valid `len` out pointer.
    let r = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 || len == 0 || len > 4096 {
        return None;
    }
    let mut buf = vec![0u8; len];
    // SAFETY: `buf` is writable and `len` bytes long.
    let r = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if r != 0 {
        return None;
    }
    buf.truncate(len);
    data_string(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_tree_strings() {
        assert_eq!(
            data_string(b"Mac mini (2024)\0").as_deref(),
            Some("Mac mini (2024)")
        );
        assert_eq!(data_string(b"Mac17,3").as_deref(), Some("Mac17,3"));
        assert_eq!(data_string(b"\0"), None);
        assert_eq!(data_string(b""), None);
    }

    #[test]
    fn host_reads() {
        let id = model_identifier().expect("hw.model");
        assert!(id.contains(','), "{id}");
        eprintln!(
            "product name {:?}, model {id:?}, battery {:?}",
            product_name(),
            internal_battery()
        );
    }
}
