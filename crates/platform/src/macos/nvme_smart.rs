//! NVMe SMART / Health Information log (log page 02h) through the public NVMeSMARTLib
//! plug-in (`IOKit/storage/nvme/NVMeSMARTLibExternal.h`), the interface smartmontools
//! uses on macOS. It works without privileges: a block device that advertises
//! `"NVMe SMART Capable" = Yes` (on Apple silicon the internal `IOEmbeddedNVMeBlockDevice`)
//! publishes the plug-in type in `IOCFPlugInTypes`; the plug-in opens a read-only user
//! client and `SMARTReadData` returns the 512-byte log. Nothing is written to the drive.
//!
//! Read once per inventory collection, never per tick. The drive accepts one SMART client
//! at a time (a second one gets `kIOReturnNoResources`, measured on the M5), so reads are
//! serialized in this process and retried briefly when another program (e.g. a disk
//! utility) holds the client.
//!
//! Health mapping, from the NVMe specification only (Base Specification, "SMART / Health
//! Information" log page):
//! - Critical Warning bit 2 (reliability degraded by media or internal errors), bit 3
//!   (media placed in read-only mode) or bit 5 (persistent memory region read-only):
//!   `Unhealthy`.
//! - Bit 0 (available spare below threshold), bit 1 (temperature outside the thresholds),
//!   bit 4 (volatile memory backup failed), Available Spare below the drive's own Available
//!   Spare Threshold, or Percentage Used ≥ 100 (the rated endurance is used up; the drive
//!   may still work): `Warning`.
//! - Otherwise `Healthy`.

use std::ffi::c_void;

use models::hardware::DiskHealthStatus;
use models::{AppError, ErrorCode};
use objc2_io_kit::io_object_t;

use super::iokit::IoObject;

/// Size of the SMART / Health Information log page.
pub const LOG_LEN: usize = 512;

/// The fields of the log page the app uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartLog {
    pub critical_warning: u8,
    /// Composite temperature in kelvin (0 = not reported).
    pub temperature_k: u16,
    /// Remaining spare capacity, percent.
    pub available_spare: u8,
    pub available_spare_threshold: u8,
    /// Vendor estimate of the endurance used, percent; may exceed 100 (capped at 255).
    pub percentage_used: u8,
    pub media_errors: u128,
}

impl SmartLog {
    /// Parses the little-endian log page (offsets from the NVMe specification).
    pub fn parse(page: &[u8; LOG_LEN]) -> Self {
        let u128_at = |o: usize| {
            let mut b = [0u8; 16];
            b.copy_from_slice(&page[o..o + 16]);
            u128::from_le_bytes(b)
        };
        Self {
            critical_warning: page[0],
            temperature_k: u16::from_le_bytes([page[1], page[2]]),
            available_spare: page[3],
            available_spare_threshold: page[4],
            percentage_used: page[5],
            media_errors: u128_at(160),
        }
    }

    pub fn health(&self) -> DiskHealthStatus {
        const UNHEALTHY_BITS: u8 = 0b0010_1100; // bits 2, 3, 5
        const WARNING_BITS: u8 = 0b0001_0011; // bits 0, 1, 4
        if self.critical_warning & UNHEALTHY_BITS != 0 {
            DiskHealthStatus::Unhealthy
        } else if self.critical_warning & WARNING_BITS != 0
            || self.available_spare < self.available_spare_threshold
            || self.percentage_used >= 100
        {
            DiskHealthStatus::Warning
        } else {
            DiskHealthStatus::Healthy
        }
    }
}

/// `CFUUIDBytes`, passed by value to `QueryInterface`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CfUuidBytes([u8; 16]);

type HResult = i32;
type IoReturn = i32;

/// `IOCFPlugInInterface`: only the `IUnknown` part is used.
#[repr(C)]
struct PlugInVtbl {
    _reserved: *mut c_void,
    query_interface:
        unsafe extern "C" fn(this: *mut c_void, iid: CfUuidBytes, out: *mut *mut c_void) -> HResult,
    _add_ref: unsafe extern "C" fn(this: *mut c_void) -> u32,
    release: unsafe extern "C" fn(this: *mut c_void) -> u32,
}

/// `IONVMeSMARTInterface` up to `SMARTReadData` (the fields after it are not used).
#[repr(C)]
struct SmartVtbl {
    _reserved: *mut c_void,
    _query_interface:
        unsafe extern "C" fn(this: *mut c_void, iid: CfUuidBytes, out: *mut *mut c_void) -> HResult,
    _add_ref: unsafe extern "C" fn(this: *mut c_void) -> u32,
    release: unsafe extern "C" fn(this: *mut c_void) -> u32,
    _version: u16,
    _revision: u16,
    smart_read_data: unsafe extern "C" fn(this: *mut c_void, data: *mut u8) -> IoReturn,
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOCreatePlugInInterfaceForService(
        service: io_object_t,
        plugin_type: *const c_void,
        interface_type: *const c_void,
        the_interface: *mut *mut *mut PlugInVtbl,
        the_score: *mut i32,
    ) -> IoReturn;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    #[allow(clippy::too_many_arguments)]
    fn CFUUIDGetConstantUUIDWithBytes(
        alloc: *const c_void,
        b0: u8,
        b1: u8,
        b2: u8,
        b3: u8,
        b4: u8,
        b5: u8,
        b6: u8,
        b7: u8,
        b8: u8,
        b9: u8,
        b10: u8,
        b11: u8,
        b12: u8,
        b13: u8,
        b14: u8,
        b15: u8,
    ) -> *const c_void;
}

/// `kIONVMeSMARTUserClientTypeID`.
const SMART_USER_CLIENT_TYPE: [u8; 16] = [
    0xAA, 0x0F, 0xA6, 0xF9, 0xC2, 0xD6, 0x45, 0x7F, 0xB1, 0x0B, 0x59, 0xA1, 0x32, 0x53, 0x29, 0x2F,
];
/// `kIONVMeSMARTInterfaceID`.
const SMART_INTERFACE: [u8; 16] = [
    0xCC, 0xD1, 0xDB, 0x19, 0xFD, 0x9A, 0x4D, 0xAF, 0xBF, 0x95, 0x12, 0x45, 0x4B, 0x23, 0x0A, 0xB6,
];
/// `kIOCFPlugInInterfaceID`.
const CF_PLUGIN_INTERFACE: [u8; 16] = [
    0xC2, 0x44, 0xE8, 0x58, 0x10, 0x9C, 0x11, 0xD4, 0x91, 0xD4, 0x00, 0x50, 0xE4, 0xC6, 0x42, 0x6F,
];

/// A constant (never released) `CFUUIDRef`.
fn constant_uuid(b: [u8; 16]) -> *const c_void {
    // SAFETY: plain value arguments; constant UUIDs are owned by CoreFoundation and live
    // for the whole process.
    unsafe {
        CFUUIDGetConstantUUIDWithBytes(
            std::ptr::null(),
            b[0],
            b[1],
            b[2],
            b[3],
            b[4],
            b[5],
            b[6],
            b[7],
            b[8],
            b[9],
            b[10],
            b[11],
            b[12],
            b[13],
            b[14],
            b[15],
        )
    }
}

fn failed(message: &str, code: i32) -> AppError {
    AppError::new(ErrorCode::Platform, message).with_detail(format!("0x{code:08X}"))
}

/// Releases a COM-style interface on drop (`this` is the `**vtable` pointer).
struct Released {
    this: *mut c_void,
    release: unsafe extern "C" fn(this: *mut c_void) -> u32,
}

impl Drop for Released {
    fn drop(&mut self) {
        // SAFETY: `this` holds the one reference this guard was created for.
        unsafe {
            (self.release)(self.this);
        }
    }
}

/// True when the block device publishes the SMART plug-in.
pub fn is_capable(device: &IoObject) -> bool {
    device.boolean("NVMe SMART Capable") == Some(true)
}

/// `kIOReturnNoResources` / `kIOReturnExclusiveAccess`: another client holds the drive.
const BUSY: [i32; 2] = [0xE000_02BE_u32 as i32, 0xE000_02C5_u32 as i32];
const ATTEMPTS: u32 = 3;
const RETRY_AFTER: std::time::Duration = std::time::Duration::from_millis(100);

/// Reads the SMART / Health Information log of an NVMe block device (the provider of an
/// `IOBlockStorageDriver`).
pub fn read(device: &IoObject) -> Result<SmartLog, AppError> {
    static ONE_CLIENT: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = ONE_CLIENT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut attempt = 1;
    loop {
        match read_once(device) {
            Err((code, _)) if BUSY.contains(&code) && attempt < ATTEMPTS => {
                attempt += 1;
                std::thread::sleep(RETRY_AFTER);
            }
            r => return r.map_err(|(_, e)| e),
        }
    }
}

fn read_once(device: &IoObject) -> Result<SmartLog, (i32, AppError)> {
    let mut plugin: *mut *mut PlugInVtbl = std::ptr::null_mut();
    let mut score = 0i32;
    // SAFETY: `device` is a live service; the UUIDs are valid constant CFUUIDs; `plugin`
    // and `score` are valid out pointers. On success `plugin` holds one reference,
    // released by the guard below.
    let r = unsafe {
        IOCreatePlugInInterfaceForService(
            device.raw(),
            constant_uuid(SMART_USER_CLIENT_TYPE),
            constant_uuid(CF_PLUGIN_INTERFACE),
            &raw mut plugin,
            &raw mut score,
        )
    };
    if r != 0 || plugin.is_null() {
        return Err((r, failed("NVMe SMART plug-in could not be created", r)));
    }
    // SAFETY: `plugin` is a non-null `IOCFPlugInInterface **` returned above.
    let plugin_vtbl = unsafe { &**plugin };
    let _plugin = Released {
        this: plugin.cast(),
        release: plugin_vtbl.release,
    };

    let mut smart: *mut c_void = std::ptr::null_mut();
    // SAFETY: `QueryInterface` of a live plug-in with a valid interface id and out
    // pointer; on success `smart` holds one reference, released by the guard below.
    let hr = unsafe {
        (plugin_vtbl.query_interface)(plugin.cast(), CfUuidBytes(SMART_INTERFACE), &raw mut smart)
    };
    if hr != 0 || smart.is_null() {
        return Err((hr, failed("NVMe SMART interface not available", hr)));
    }
    // SAFETY: `smart` is an `IONVMeSMARTInterface **` (the interface id asked for).
    let smart_vtbl = unsafe { &**smart.cast::<*mut SmartVtbl>() };
    let _smart = Released {
        this: smart,
        release: smart_vtbl.release,
    };

    let mut page = [0u8; LOG_LEN];
    // SAFETY: `SMARTReadData` writes exactly one 512-byte log page into `page`.
    let r = unsafe { (smart_vtbl.smart_read_data)(smart, page.as_mut_ptr()) };
    if r != 0 {
        return Err((r, failed("NVMe SMART log could not be read", r)));
    }
    Ok(SmartLog::parse(&page))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(f: impl FnOnce(&mut [u8; LOG_LEN])) -> SmartLog {
        let mut p = [0u8; LOG_LEN];
        p[1..3].copy_from_slice(&310u16.to_le_bytes()); // 37 °C
        p[3] = 100;
        p[4] = 10;
        f(&mut p);
        SmartLog::parse(&p)
    }

    #[test]
    fn parses_the_spec_offsets() {
        let l = page(|p| {
            p[0] = 0x04;
            p[5] = 7;
            p[160] = 3;
            p[175] = 1; // high byte of the 128-bit counter
        });
        assert_eq!(l.critical_warning, 4);
        assert_eq!(l.temperature_k, 310);
        assert_eq!((l.available_spare, l.available_spare_threshold), (100, 10));
        assert_eq!(l.percentage_used, 7);
        assert_eq!(l.media_errors, 3 + (1u128 << 120));
    }

    #[test]
    fn health_follows_the_specification() {
        assert_eq!(page(|_| {}).health(), DiskHealthStatus::Healthy);
        for bit in [0, 1, 4] {
            assert_eq!(
                page(|p| p[0] = 1 << bit).health(),
                DiskHealthStatus::Warning,
                "bit {bit}"
            );
        }
        for bit in [2, 3, 5] {
            assert_eq!(
                page(|p| p[0] = 1 << bit).health(),
                DiskHealthStatus::Unhealthy,
                "bit {bit}"
            );
        }
        // Spare below the drive's own threshold, without the warning bit set.
        assert_eq!(page(|p| p[3] = 9).health(), DiskHealthStatus::Warning);
        assert_eq!(page(|p| p[3] = 10).health(), DiskHealthStatus::Healthy);
        // Endurance used up.
        assert_eq!(page(|p| p[5] = 99).health(), DiskHealthStatus::Healthy);
        assert_eq!(page(|p| p[5] = 100).health(), DiskHealthStatus::Warning);
        // Unhealthy wins over warning.
        assert_eq!(
            page(|p| p[0] = 0b1001).health(),
            DiskHealthStatus::Unhealthy
        );
    }

    // The internal SSD of this Mac, read without privileges.
    #[test]
    fn host_internal_ssd_log_is_readable_without_root() {
        let drivers = super::super::iokit::services_of_class(c"IOBlockStorageDriver")
            .expect("block storage drivers");
        let devices: Vec<IoObject> = drivers
            .iter()
            .filter_map(IoObject::parent)
            .filter(is_capable)
            .collect();
        if cfg!(target_arch = "aarch64") {
            assert!(!devices.is_empty(), "the internal SSD is SMART capable");
        }
        for d in &devices {
            let l = read(d).expect("SMART log");
            // A drive in service reports a plausible temperature and spare.
            assert!((273..373).contains(&l.temperature_k), "{l:?}");
            assert!(l.available_spare <= 100, "{l:?}");
            eprintln!("{l:?} -> {:?}", l.health());
        }
    }
}
