//! Size and last use of a macOS app bundle from Spotlight metadata (`MDItem`, CoreServices):
//! `kMDItemPhysicalSize` (or `kMDItemLogicalSize`) and `kMDItemLastUsedDate`, which Launch
//! Services updates when the app is opened. Read only, no privileges. Both stay `None` when
//! the volume is not indexed or the attribute was never recorded: nothing is estimated.

use std::ffi::c_void;
use std::path::Path;
use std::ptr::NonNull;

use objc2_core_foundation::{CFNumber, CFRetained, CFString, CFType};

/// Seconds from the Unix epoch to the Core Foundation epoch (2001-01-01 UTC).
const CF_EPOCH_UNIX_SECS: f64 = 978_307_200.0;

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    /// +1 `MDItemRef`, or NULL when the path is not known to Spotlight.
    fn MDItemCreate(allocator: *const c_void, path: *const c_void) -> *mut c_void;
    /// +1 CF value of the attribute, or NULL.
    fn MDItemCopyAttribute(item: *const c_void, name: *const c_void) -> *mut c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFGetTypeID(cf: *const c_void) -> usize;
    fn CFDateGetTypeID() -> usize;
    fn CFDateGetAbsoluteTime(date: *const c_void) -> f64;
}

fn cf_ptr<T: ?Sized>(v: &T) -> *const c_void {
    (v as *const T).cast()
}

/// What Spotlight knows about one bundle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BundleUsage {
    pub size_bytes: Option<u64>,
    /// Unix ms.
    pub last_used_ms: Option<u64>,
}

struct Item(CFRetained<CFType>);

impl Item {
    fn open(path: &Path) -> Option<Self> {
        let p = CFString::from_str(&path.to_string_lossy());
        // SAFETY: valid CFString alive for the call; NULL allocator = default. Create rule.
        let raw = unsafe { MDItemCreate(std::ptr::null(), cf_ptr(&*p)) };
        let ptr = NonNull::new(raw.cast::<CFType>())?;
        // SAFETY: `ptr` is a +1 reference we now own.
        Some(Self(unsafe { CFRetained::from_raw(ptr) }))
    }

    fn attribute(&self, name: &str) -> Option<CFRetained<CFType>> {
        let n = CFString::from_str(name);
        // SAFETY: the item and the name are valid CF objects alive for the call. Copy rule.
        let raw = unsafe { MDItemCopyAttribute(cf_ptr(&*self.0), cf_ptr(&*n)) };
        let ptr = NonNull::new(raw.cast::<CFType>())?;
        // SAFETY: `ptr` is a +1 reference we now own.
        Some(unsafe { CFRetained::from_raw(ptr) })
    }

    fn number(&self, name: &str) -> Option<u64> {
        let v = self.attribute(name)?;
        let n = v.downcast_ref::<CFNumber>()?;
        n.as_i64()
            .and_then(|x| u64::try_from(x).ok())
            .filter(|&x| x > 0)
    }

    fn date_ms(&self, name: &str) -> Option<u64> {
        let v = self.attribute(name)?;
        // SAFETY: plain type-id queries on a live CF object.
        if unsafe { CFGetTypeID(cf_ptr(&*v)) != CFDateGetTypeID() } {
            return None;
        }
        // SAFETY: `v` is a live CFDate (checked above).
        let abs = unsafe { CFDateGetAbsoluteTime(cf_ptr(&*v)) };
        unix_ms(abs)
    }
}

/// Core Foundation absolute time (s since 2001) to Unix ms; `None` before 1970 or non-finite.
fn unix_ms(abs: f64) -> Option<u64> {
    let ms = (abs + CF_EPOCH_UNIX_SECS) * 1000.0;
    // Truncation is fine: well inside u64 for any real date.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (ms.is_finite() && ms > 0.0).then_some(ms as u64)
}

/// Size and last use of `bundle`, as far as Spotlight recorded them.
pub fn bundle_usage(bundle: &Path) -> BundleUsage {
    let Some(item) = Item::open(bundle) else {
        return BundleUsage::default();
    };
    BundleUsage {
        size_bytes: item
            .number("kMDItemPhysicalSize")
            .or_else(|| item.number("kMDItemLogicalSize")),
        last_used_ms: item.date_ms("kMDItemLastUsedDate"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cf_time_converts_to_unix_ms() {
        assert_eq!(unix_ms(0.0), Some(978_307_200_000));
        assert_eq!(unix_ms(-CF_EPOCH_UNIX_SECS - 1.0), None);
        assert_eq!(unix_ms(f64::NAN), None);
    }

    #[test]
    fn missing_path_gives_nothing() {
        let u = bundle_usage(Path::new("/nonexistent/Nothing Here.app"));
        assert_eq!(u, BundleUsage::default());
    }

    /// Prints what Spotlight reports for the system's own apps (run with `--ignored`).
    #[test]
    #[ignore = "reads this Mac's Spotlight index"]
    fn print_system_app_usage() {
        for app in [
            "/System/Applications/Calculator.app",
            "/Applications/Safari.app",
        ] {
            println!("{app}: {:?}", bundle_usage(Path::new(app)));
        }
    }
}
