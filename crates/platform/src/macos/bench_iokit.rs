//! Small, safe wrappers over the I/O Kit registry calls the benchmark providers need
//! (`bench_env`, `gpu_bench`). Read-only: nothing here opens a device or changes a property.

use objc2_core_foundation::{
    CFBoolean, CFData, CFDictionary, CFNumber, CFRetained, CFString, CFType,
};
use objc2_io_kit::{
    IOBSDNameMatching, IOIteratorNext, IOObjectConformsTo, IOObjectRelease,
    IORegistryEntryCreateCFProperty, IORegistryEntryGetParentIterator,
    IORegistryEntryGetRegistryEntryID, IOServiceGetMatchingService, IOServiceGetMatchingServices,
    IOServiceMatching, io_iterator_t, io_object_t, kIOReturnSuccess, kIOServicePlane,
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

    /// The registry entry id (global, stable for the life of the entry).
    pub(crate) fn registry_id(&self) -> Option<u64> {
        let mut id = 0u64;
        // SAFETY: `self.0` is a live registry entry; `id` is a valid out pointer.
        let rc = unsafe { IORegistryEntryGetRegistryEntryID(self.0, &mut id) };
        (rc == kIOReturnSuccess && id != 0).then_some(id)
    }

    /// True if the object is an instance of `class` or a subclass.
    pub(crate) fn conforms_to(&self, class: &std::ffi::CStr) -> bool {
        // SAFETY: `self.0` is live; `class` is NUL-terminated and outlives the call (I/O Kit
        // only reads it as a C string despite the `io_name_t` pointer type).
        unsafe { IOObjectConformsTo(self.0, class.as_ptr().cast_mut().cast()) }
    }

    /// The raw property `key` of this entry (not searched in parents).
    pub(crate) fn property(&self, key: &str) -> Option<CFRetained<CFType>> {
        let key = CFString::from_str(key);
        // SAFETY: `self.0` is live; `key` is a valid CFString for the call; default allocator.
        unsafe { IORegistryEntryCreateCFProperty(self.0, Some(&key), None, 0) }
    }

    pub(crate) fn string(&self, key: &str) -> Option<String> {
        let v = self.property(key)?;
        let s = v.downcast::<CFString>().ok()?.to_string();
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_owned())
    }

    pub(crate) fn number(&self, key: &str) -> Option<i64> {
        self.property(key)?.downcast::<CFNumber>().ok()?.as_i64()
    }

    pub(crate) fn boolean(&self, key: &str) -> Option<bool> {
        Some(self.property(key)?.downcast::<CFBoolean>().ok()?.as_bool())
    }

    /// First 4 bytes of a data property as a little-endian u32 (e.g. `vendor-id`).
    pub(crate) fn data_u32(&self, key: &str) -> Option<u32> {
        let v = self.property(key)?.downcast::<CFData>().ok()?.to_vec();
        let b: [u8; 4] = v.get(..4)?.try_into().ok()?;
        Some(u32::from_le_bytes(b))
    }

    /// A NUL-terminated string stored as data (e.g. the `model` of an `IOPCIDevice`).
    pub(crate) fn data_string(&self, key: &str) -> Option<String> {
        let v = self.property(key)?.downcast::<CFData>().ok()?.to_vec();
        let end = v.iter().position(|b| *b == 0).unwrap_or(v.len());
        let s = String::from_utf8_lossy(&v[..end]);
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_owned())
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

/// Every registered service of `class` (e.g. `IOAccelerator`).
pub(crate) fn services_of_class(class: &std::ffi::CStr) -> Vec<IoObject> {
    // SAFETY: `class` is NUL-terminated; the returned dictionary is owned.
    let Some(matching) = (unsafe { IOServiceMatching(class.as_ptr()) }) else {
        return Vec::new();
    };
    let mut it: io_iterator_t = 0;
    // SAFETY: the dictionary is consumed by the call; `it` is a valid out pointer.
    let rc = unsafe {
        IOServiceGetMatchingServices(
            MAIN_PORT,
            Some(CFRetained::<CFDictionary>::from(&matching)),
            &mut it,
        )
    };
    if rc != kIOReturnSuccess {
        return Vec::new();
    }
    IoObject::new(it).map(|it| drain(&it)).unwrap_or_default()
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
