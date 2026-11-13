//! Read-only access to the Apple System Management Controller (SMC) through the public
//! `AppleSMC` I/O Kit user client, as Activity-style monitors do. Any user may open it and
//! read keys; writing a key (forcing a fan) needs root, and this module never writes.
//!
//! Protocol (stable since the Intel Macs, unchanged on Apple silicon): one struct method,
//! selector 2 (`kSMCHandleYPCEvent`), exchanging an 80-byte `SMCKeyData` record. Command 9
//! reads a key's type and size, command 5 reads its bytes. Fan keys: `FNum` (fan count),
//! `F<n>Ac` (actual RPM), `F<n>Mn` / `F<n>Mx` (min / max), `F<n>Tg` (target), `F<n>Md`
//! (mode: 0 auto, 1 forced). RPMs are `flt ` (f32, little endian) on Apple silicon and
//! `fpe2` (unsigned 14.2 fixed point, big endian) on Intel Macs.
//!
//! The connection is closed on drop (`IOServiceClose`), so no user client leaks, and the
//! service object is released by `IoObject`.

use std::ffi::c_void;

use crate::macos::iokit::{IoObject, services_of_class};

/// I/O Kit `kern_return_t`.
type KernReturn = i32;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceOpen(service: u32, owning_task: u32, kind: u32, connect: *mut u32) -> KernReturn;
    fn IOServiceClose(connect: u32) -> KernReturn;
    fn IOConnectCallStructMethod(
        connection: u32,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> KernReturn;
}

unsafe extern "C" {
    /// The task's own port (what `mach_task_self()` returns), from libSystem.
    static mach_task_self_: u32;
}

const KERNEL_INDEX_SMC: u32 = 2;
const CMD_READ_BYTES: u8 = 5;
const CMD_READ_KEYINFO: u8 = 9;
/// `SMCKeyData.result` when the key does not exist.
const RESULT_KEY_NOT_FOUND: u8 = 0x84;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Vers {
    major: u8,
    minor: u8,
    build: u8,
    reserved: u8,
    release: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PLimit {
    version: u16,
    length: u16,
    cpu: u32,
    gpu: u32,
    mem: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KeyInfo {
    data_size: u32,
    data_type: u32,
    attributes: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KeyData {
    key: u32,
    vers: Vers,
    p_limit: PLimit,
    info: KeyInfo,
    result: u8,
    status: u8,
    data8: u8,
    data32: u32,
    bytes: [u8; 32],
}

const _: () = assert!(std::mem::size_of::<KeyData>() == 80);

/// Four-character code as the SMC stores it (big-endian u32).
pub fn fourcc(code: &str) -> Option<u32> {
    let b = code.as_bytes();
    (b.len() == 4).then(|| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// A key's raw value.
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    pub data_type: [u8; 4],
    pub bytes: Vec<u8>,
}

impl Value {
    /// Decodes the common numeric SMC types.
    pub fn as_f64(&self) -> Option<f64> {
        let b = &self.bytes;
        match &self.data_type {
            b"flt " if b.len() == 4 => {
                Some(f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])))
            }
            b"fpe2" if b.len() == 2 => Some(f64::from(u16::from_be_bytes([b[0], b[1]])) / 4.0),
            b"ui8 " | b"flag" if b.len() == 1 => Some(f64::from(b[0])),
            b"ui16" if b.len() == 2 => Some(f64::from(u16::from_be_bytes([b[0], b[1]]))),
            b"ui32" if b.len() == 4 => {
                Some(f64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])))
            }
            b"sp78" if b.len() == 2 => Some(f64::from(i16::from_be_bytes([b[0], b[1]])) / 256.0),
            _ => None,
        }
    }
}

/// Why a key could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// The SMC has no such key on this Mac.
    Missing,
    Failed(String),
}

/// An open `AppleSMC` user client, closed on drop.
pub struct Smc {
    connection: u32,
    _service: IoObject,
}

impl Smc {
    /// Opens the user client of the first `AppleSMC` service. Needs no privilege.
    pub fn open() -> Result<Self, String> {
        let service = services_of_class(c"AppleSMC")
            .map_err(|e| e.message)?
            .into_iter()
            .next()
            .ok_or_else(|| "no AppleSMC service in the I/O Registry".to_owned())?;
        let mut connection = 0u32;
        // SAFETY: `service` is a live service reference owned by `service`; the task port is
        // the process's own (an immutable libSystem static); `connection` is a writable u32.
        let r = unsafe { IOServiceOpen(service.raw(), mach_task_self_, 0, &raw mut connection) };
        if r != 0 || connection == 0 {
            return Err(format!(
                "IOServiceOpen(AppleSMC) failed (kern_return {r:#x})"
            ));
        }
        Ok(Self {
            connection,
            _service: service,
        })
    }

    fn call(&self, input: &KeyData) -> Result<KeyData, String> {
        let mut output = KeyData::default();
        let mut out_size = std::mem::size_of::<KeyData>();
        // SAFETY: both buffers are `repr(C)` 80-byte records owned here, sizes match them,
        // and the connection is open for the life of `self`.
        let r = unsafe {
            IOConnectCallStructMethod(
                self.connection,
                KERNEL_INDEX_SMC,
                std::ptr::from_ref(input).cast(),
                std::mem::size_of::<KeyData>(),
                (&raw mut output).cast(),
                &raw mut out_size,
            )
        };
        if r != 0 {
            return Err(format!("SMC call failed (kern_return {r:#x})"));
        }
        Ok(output)
    }

    /// Reads one key. Never writes.
    pub fn read(&self, key: &str) -> Result<Value, ReadError> {
        let code = fourcc(key).ok_or_else(|| ReadError::Failed(format!("bad key {key:?}")))?;
        let info = self
            .call(&KeyData {
                key: code,
                data8: CMD_READ_KEYINFO,
                ..KeyData::default()
            })
            .map_err(ReadError::Failed)?;
        if info.result == RESULT_KEY_NOT_FOUND {
            return Err(ReadError::Missing);
        }
        if info.result != 0 {
            return Err(ReadError::Failed(format!(
                "SMC key {key} info result {:#x}",
                info.result
            )));
        }
        let size = info.info.data_size as usize;
        if size == 0 || size > 32 {
            return Err(ReadError::Failed(format!("SMC key {key} size {size}")));
        }
        let out = self
            .call(&KeyData {
                key: code,
                info: info.info,
                data8: CMD_READ_BYTES,
                ..KeyData::default()
            })
            .map_err(ReadError::Failed)?;
        if out.result == RESULT_KEY_NOT_FOUND {
            return Err(ReadError::Missing);
        }
        if out.result != 0 {
            return Err(ReadError::Failed(format!(
                "SMC key {key} read result {:#x}",
                out.result
            )));
        }
        Ok(Value {
            data_type: info.info.data_type.to_be_bytes(),
            bytes: out.bytes[..size].to_vec(),
        })
    }

    pub fn read_f64(&self, key: &str) -> Option<f64> {
        self.read(key).ok()?.as_f64().filter(|v| v.is_finite())
    }
}

impl Drop for Smc {
    fn drop(&mut self) {
        // SAFETY: the connection was opened by `open` and is closed exactly once here.
        let _ = unsafe { IOServiceClose(self.connection) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_decode() {
        let v = |t: &[u8; 4], b: &[u8]| Value {
            data_type: *t,
            bytes: b.to_vec(),
        };
        assert_eq!(v(b"flt ", &1200.0f32.to_le_bytes()).as_f64(), Some(1200.0));
        assert_eq!(v(b"fpe2", &[0x12, 0xc0]).as_f64(), Some(1200.0));
        assert_eq!(v(b"ui8 ", &[2]).as_f64(), Some(2.0));
        assert_eq!(v(b"sp78", &[0x2a, 0x80]).as_f64(), Some(42.5));
        assert_eq!(v(b"ch8*", &[1]).as_f64(), None);
        assert_eq!(v(b"flt ", &[1, 2]).as_f64(), None);
        assert_eq!(fourcc("FNum"), Some(0x464e_756d));
        assert_eq!(fourcc("FNu"), None);
    }

    #[test]
    fn host_smc_opens_and_missing_keys_are_reported() {
        if !crate::macos::physical_apple_silicon() {
            return;
        }
        let smc = Smc::open().expect("AppleSMC user client opens without root");
        assert_eq!(smc.read("Zz9!"), Err(ReadError::Missing));
    }
}
