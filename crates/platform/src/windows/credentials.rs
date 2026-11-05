//! Generic credentials in Windows Credential Manager (`CredWriteW` / `CredReadW` /
//! `CredDeleteW`). The secret is never logged or put in an error.

use ::windows::Win32::Foundation::ERROR_NOT_FOUND;
use ::windows::Win32::Security::Credentials::{
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
    CredWriteW,
};
use ::windows::core::{HRESULT, PCWSTR, PWSTR};
use models::{AppError, ErrorCode};

/// Shown as "Nome utente" in the Control Panel's credential list.
const USER_NAME: &str = "PC Diagnostics";
/// `CRED_MAX_CREDENTIAL_BLOB_SIZE` (5 × 512 bytes).
const MAX_BLOB: usize = 5 * 512;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn platform_error(op: &str, e: &::windows::core::Error) -> AppError {
    AppError::new(
        ErrorCode::Platform,
        format!("Windows Credential Manager: {op} failed"),
    )
    .with_detail(format!("HRESULT 0x{:08X}", e.code().0))
}

fn is_not_found(e: &::windows::core::Error) -> bool {
    e.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0)
}

/// Frees a `CredReadW` result exactly once.
struct CredBuf(*mut CREDENTIALW);

impl Drop for CredBuf {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a successful CredReadW and is freed only here.
        unsafe { CredFree(self.0.cast_const().cast()) };
    }
}

/// Creates or replaces the generic credential `target` with `secret`.
pub fn write(target: &str, secret: &[u8]) -> Result<(), AppError> {
    if secret.len() > MAX_BLOB {
        return Err(AppError::invalid_input("the credential is too large"));
    }
    let mut target_w = wide(target);
    let mut user_w = wide(USER_NAME);
    let cred = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target_w.as_mut_ptr()),
        CredentialBlobSize: u32::try_from(secret.len()).unwrap_or(0),
        // CredWriteW only reads the blob; the mutable pointer is the API's signature.
        CredentialBlob: secret.as_ptr().cast_mut(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(user_w.as_mut_ptr()),
        ..Default::default()
    };
    // SAFETY: every pointer in `cred` refers to a live, NUL-terminated (strings) or
    // `CredentialBlobSize`-long (blob) buffer that outlives the call.
    unsafe { CredWriteW(&cred, 0) }.map_err(|e| platform_error("write", &e))
}

/// The blob of the generic credential `target`, `None` if absent. The copy inside the
/// API's buffer is zeroed before it is freed; the caller owns (and should zero) the result.
pub fn read(target: &str) -> Result<Option<Vec<u8>>, AppError> {
    let target_w = wide(target);
    let mut out: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: NUL-terminated target; `out` is a valid out pointer, freed by CredBuf.
    match unsafe { CredReadW(PCWSTR(target_w.as_ptr()), CRED_TYPE_GENERIC, None, &mut out) } {
        Ok(()) => {}
        Err(e) if is_not_found(&e) => return Ok(None),
        Err(e) => return Err(platform_error("read", &e)),
    }
    if out.is_null() {
        return Ok(None);
    }
    let buf = CredBuf(out);
    // SAFETY: CredReadW succeeded, so `out` points to a valid CREDENTIALW that stays
    // allocated until `buf` is dropped.
    let cred = unsafe { &*buf.0 };
    let len = cred.CredentialBlobSize as usize;
    if cred.CredentialBlob.is_null() || len == 0 {
        return Ok(Some(Vec::new()));
    }
    // SAFETY: the API guarantees `CredentialBlob` holds `CredentialBlobSize` bytes, owned
    // by the allocation in `buf` (which is ours to read and overwrite until CredFree).
    let blob = unsafe { std::slice::from_raw_parts_mut(cred.CredentialBlob, len) };
    let copy = blob.to_vec();
    crate::credentials::zeroize(blob);
    Ok(Some(copy))
}

/// Deletes the generic credential `target`. Absent is success.
pub fn delete(target: &str) -> Result<(), AppError> {
    let target_w = wide(target);
    // SAFETY: NUL-terminated target that outlives the call.
    match unsafe { CredDeleteW(PCWSTR(target_w.as_ptr()), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(()),
        Err(e) if is_not_found(&e) => Ok(()),
        Err(e) => Err(platform_error("delete", &e)),
    }
}
