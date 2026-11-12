//! Small, safe wrappers over the I/O Kit registry calls the benchmark providers need
//! (`bench_env`). Read-only: nothing here opens a device or changes a property.

use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_io_kit::{
    IOBSDNameMatching, IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperty,
    IORegistryEntryGetParentIterator, IOServiceGetMatchingService, io_iterator_t, io_object_t,
    kIOReturnSuccess, kIOServicePlane,
};

/// `kIOMainPortDefault` is defined as `MACH_PORT_NULL`; passing 0 avoids reading the
/// extern static.
const MAIN_PORT: libc::mach_port_t = 0;

/// An owned I/O Kit object reference, released on drop.
pub(crate) struct IoObject(io_object_t);

impl IoObject {
    fn new(raw: io_object_t) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    pub(crate) fn raw(&self) -> io_object_t {
        self.0
    }

    /// The raw property `key` of this entry (not searched in parents).
    pub(crate) fn property(&self, key: &str) -> Option<CFRetained<CFType>> {
        let key = CFString::from_str(key);
        // SAFETY: `self.0` is live; `key` is a valid CFString for the call; default allocator.
        unsafe { IORegistryEntryCreateCFProperty(self.0, Some(&key), None, 0) }
    }

    pub(crate) fn number(&self, key: &str) -> Option<i64> {
        self.property(key)?.downcast::<CFNumber>().ok()?.as_i64()
    }

    pub(crate) fn boolean(&self, key: &str) -> Option<bool> {
        Some(self.property(key)?.downcast::<CFBoolean>().ok()?.as_bool())
    }

    /// A string value of the dictionary property `dict_key`.
    pub(crate) fn dict_string(&self, dict_key: &str, key: &str) -> Option<String> {
        let d = self.property(dict_key)?.downcast::<CFDictionary>().ok()?;
        // SAFETY: registry property dictionaries have CFString keys; values are only
        // used as `CFType` and downcast with a type check.
        let d = unsafe { d.cast_unchecked::<CFString, CFType>() };
        let s = d
            .get(&CFString::from_str(key))?
            .downcast::<CFString>()
            .ok()?
            .to_string();
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_owned())
    }

    /// The parents of this entry in the service plane.
    pub(crate) fn parents(&self) -> Vec<IoObject> {
        let mut it: io_iterator_t = 0;
        // SAFETY: `self.0` is live; the plane name is NUL-terminated; `it` is a valid out
        // pointer, released by `IoObject` below.
        let rc = unsafe {
            IORegistryEntryGetParentIterator(
                self.0,
                kIOServicePlane.as_ptr().cast_mut().cast(),
                &mut it,
            )
        };
        if rc != kIOReturnSuccess {
            return Vec::new();
        }
        IoObject::new(it).map(|it| drain(&it)).unwrap_or_default()
    }
}

impl Drop for IoObject {
    fn drop(&mut self) {
        let _ = IOObjectRelease(self.0);
    }
}

fn drain(it: &IoObject) -> Vec<IoObject> {
    std::iter::from_fn(|| IoObject::new(IOIteratorNext(it.raw())))
        .take(256)
        .collect()
}

/// The `IOMedia` for a BSD device name such as `disk3s5`.
pub(crate) fn media_for_bsd_name(bsd: &str) -> Option<IoObject> {
    if bsd.is_empty() || !bsd.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let name = std::ffi::CString::new(bsd).ok()?;
    // SAFETY: `name` is NUL-terminated and alive for the call; the returned dictionary is
    // owned (and consumed by `IOServiceGetMatchingService`).
    let matching = unsafe { IOBSDNameMatching(MAIN_PORT, 0, name.as_ptr()) }?;
    // SAFETY: the matching dictionary is consumed; the returned service is owned.
    let service = unsafe {
        IOServiceGetMatchingService(MAIN_PORT, Some(CFRetained::<CFDictionary>::from(&matching)))
    };
    IoObject::new(service)
}

/// Walks up the service plane from `start` (included) until `f` returns `Some`. Stops with
/// `None` at an entry with several parents (e.g. a volume spanning more than one device),
/// at the root, or after 64 levels.
pub(crate) fn find_in_ancestors<T>(
    start: IoObject,
    f: impl Fn(&IoObject) -> Option<T>,
) -> Option<T> {
    let mut cur = start;
    for _ in 0..64 {
        if let Some(v) = f(&cur) {
            return Some(v);
        }
        let mut parents = cur.parents();
        if parents.len() != 1 {
            return None;
        }
        cur = parents.pop()?;
    }
    None
}
