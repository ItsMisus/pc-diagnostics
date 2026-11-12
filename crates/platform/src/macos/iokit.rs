//! Small owned wrappers over the public I/O Kit registry calls used by the macOS
//! providers (GPU, block storage, power manager tables). No privileges are needed: the
//! registry is readable by any user. Nothing here opens a user client to a driver.

use std::ffi::{CStr, c_char};

use models::{AppError, ErrorCode};
use objc2_core_foundation::{CFData, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_io_kit::{
    IOIteratorNext, IOObjectConformsTo, IOObjectGetClass, IOObjectRelease,
    IORegistryEntryCreateCFProperty, IORegistryEntryGetChildEntry, IORegistryEntryGetName,
    IORegistryEntryGetParentEntry, IORegistryEntryGetRegistryEntryID, IOServiceGetMatchingServices,
    IOServiceMatching, io_iterator_t, io_object_t, kIOMainPortDefault, kIOReturnSuccess,
    kIOServicePlane,
};

/// An owned I/O Kit object reference (one user reference on a Mach port name), released
/// on drop.
#[derive(Debug)]
pub struct IoObject(io_object_t);

impl IoObject {
    fn from_raw(raw: io_object_t) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    /// Registry entry id: unique for the entry's lifetime (until it is detached or the
    /// machine reboots), the same value Metal reports as `MTLDevice.registryID`.
    pub fn entry_id(&self) -> Option<u64> {
        let mut id = 0u64;
        // SAFETY: `self.0` is a live registry entry reference; `id` is a valid out pointer.
        let r = unsafe { IORegistryEntryGetRegistryEntryID(self.0, &raw mut id) };
        (r == kIOReturnSuccess).then_some(id)
    }

    /// Provider (parent) in the service plane.
    pub fn parent(&self) -> Option<Self> {
        let mut parent: io_object_t = 0;
        // SAFETY: live entry; the plane name is a NUL-terminated static string that the
        // call only reads (the binding types it as a mutable `io_name_t`); `parent` is a
        // valid out pointer and receives a new reference that `IoObject` releases.
        let r = unsafe {
            IORegistryEntryGetParentEntry(self.0, plane_ptr(kIOServicePlane), &raw mut parent)
        };
        if r == kIOReturnSuccess {
            Self::from_raw(parent)
        } else {
            None
        }
    }

    /// First client (child) in the service plane.
    pub fn first_child(&self) -> Option<Self> {
        let mut child: io_object_t = 0;
        // SAFETY: as in `parent`.
        let r = unsafe {
            IORegistryEntryGetChildEntry(self.0, plane_ptr(kIOServicePlane), &raw mut child)
        };
        if r == kIOReturnSuccess {
            Self::from_raw(child)
        } else {
            None
        }
    }

    /// Registry name of the entry (for device-tree nubs, the node name without `@unit`).
    pub fn name(&self) -> Option<String> {
        let mut buf: [c_char; 128] = [0; 128];
        // SAFETY: live entry; `buf` is the 128-byte `io_name_t` buffer the call fills with
        // a NUL-terminated string.
        let r = unsafe { IORegistryEntryGetName(self.0, &raw mut buf) };
        (r == kIOReturnSuccess).then(|| c_buf_to_string(&buf))
    }

    /// Kernel class name of the object (e.g. `IOEmbeddedNVMeBlockDevice`).
    pub fn class_name(&self) -> Option<String> {
        let mut buf: [c_char; 128] = [0; 128];
        // SAFETY: as in `name`.
        let r = unsafe { IOObjectGetClass(self.0, &raw mut buf) };
        (r == kIOReturnSuccess).then(|| c_buf_to_string(&buf))
    }

    /// Whether the object is an instance of `class` or of a subclass.
    pub fn conforms_to(&self, class: &CStr) -> bool {
        // SAFETY: live object; `class` is NUL-terminated and only read.
        unsafe { IOObjectConformsTo(self.0, class.as_ptr().cast_mut().cast()) }
    }

    /// A copy of one registry property (a snapshot: later changes are not reflected).
    pub fn property(&self, key: &str) -> Option<CFRetained<CFType>> {
        let key = CFString::from_str(key);
        // SAFETY: live entry; `key` is a valid CFString; `None` selects the default
        // allocator. The binding takes ownership of the +1 "Create" result.
        unsafe { IORegistryEntryCreateCFProperty(self.0, Some(&key), None, 0) }
    }

    pub fn dictionary(&self, key: &str) -> Option<Dict> {
        self.property(key)?
            .downcast::<CFDictionary>()
            .ok()
            .map(Dict)
    }

    pub fn string(&self, key: &str) -> Option<String> {
        cf_string(&*self.property(key)?)
    }

    pub fn number(&self, key: &str) -> Option<i64> {
        cf_i64(&*self.property(key)?)
    }

    pub fn data(&self, key: &str) -> Option<Vec<u8>> {
        Some(self.property(key)?.downcast::<CFData>().ok()?.to_vec())
    }
}

impl Drop for IoObject {
    fn drop(&mut self) {
        let _ = IOObjectRelease(self.0);
    }
}

/// A registry dictionary property (`OSDictionary` → `CFDictionary`, string keys).
pub struct Dict(CFRetained<CFDictionary>);

impl Dict {
    pub fn get(&self, key: &str) -> Option<CFRetained<CFType>> {
        // SAFETY: registry dictionaries are serialized `OSDictionary`s, whose keys are
        // always strings (`OSSymbol` → `CFString`); every value is some CF object, so
        // `CFType` is a valid view of it.
        let typed = unsafe { self.0.cast_unchecked::<CFString, CFType>() };
        typed.get(&CFString::from_str(key))
    }

    pub fn number(&self, key: &str) -> Option<i64> {
        cf_i64(&*self.get(key)?)
    }

    pub fn string(&self, key: &str) -> Option<String> {
        cf_string(&*self.get(key)?)
    }
}

fn cf_i64(v: &CFType) -> Option<i64> {
    v.downcast_ref::<CFNumber>()?.as_i64()
}

fn cf_string(v: &CFType) -> Option<String> {
    Some(v.downcast_ref::<CFString>()?.to_string())
}

fn c_buf_to_string(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| c.to_ne_bytes()[0])
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn plane_ptr(plane: &CStr) -> *mut [c_char; 128] {
    plane.as_ptr().cast_mut().cast()
}

/// All registered services of `class` (and its subclasses).
pub fn services_of_class(class: &CStr) -> Result<Vec<IoObject>, AppError> {
    // SAFETY: `class` is NUL-terminated; the result is an owned (+1) dictionary.
    let Some(matching) = (unsafe { IOServiceMatching(class.as_ptr()) }) else {
        return Err(AppError::new(
            ErrorCode::Platform,
            "IOServiceMatching returned no dictionary",
        )
        .with_detail(class.to_string_lossy().into_owned()));
    };
    let matching = CFRetained::<CFDictionary>::from(&matching);
    let mut iter: io_iterator_t = 0;
    // SAFETY: `kIOMainPortDefault` is an immutable extern static (the default main port,
    // 0); the call consumes the matching dictionary and writes an owned iterator.
    let r =
        unsafe { IOServiceGetMatchingServices(kIOMainPortDefault, Some(matching), &raw mut iter) };
    if r != kIOReturnSuccess {
        return Err(
            AppError::new(ErrorCode::Platform, "IOServiceGetMatchingServices failed")
                .with_detail(format!("{} (kern_return {r:#x})", class.to_string_lossy())),
        );
    }
    let Some(iter) = IoObject::from_raw(iter) else {
        return Ok(Vec::new()); // success with no match
    };
    let mut out = Vec::new();
    while let Some(o) = IoObject::from_raw(IOIteratorNext(iter.0)) {
        out.push(o);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_buffer_stops_at_nul() {
        let mut buf: [c_char; 8] = [0; 8];
        for (i, b) in b"pmgr".iter().enumerate() {
            buf[i] = c_char::from_ne_bytes([*b]);
        }
        assert_eq!(c_buf_to_string(&buf), "pmgr");
    }

    #[test]
    fn host_registry_has_platform_services() {
        let accel = services_of_class(c"IOAccelerator").expect("IOAccelerator lookup");
        assert!(!accel.is_empty(), "every supported Mac has a GPU");
        let a = &accel[0];
        assert!(a.entry_id().is_some_and(|id| id > 0));
        assert!(a.class_name().is_some_and(|c| !c.is_empty()));
        assert!(a.parent().is_some());
        assert!(a.conforms_to(c"IOAccelerator"));
    }
}
