//! Generic passwords in the user's login Keychain (Security framework `SecItemAdd` /
//! `SecItemCopyMatching` / `SecItemUpdate` / `SecItemDelete` through
//! `security-framework`). The secret is never logged or put in an error.
//!
//! The item is a generic password with service = the credential target name and a fixed
//! account ([`ACCOUNT`]). It is visible in Keychain Access under that service name. The
//! Keychain encrypts it with the user's login keychain key; nothing is written to disk by
//! the app itself.

use models::{AppError, ErrorCode};
use security_framework::base::Error as SecError;
use security_framework::passwords;

/// Account attribute of the item (fixed: one key per user).
pub const ACCOUNT: &str = "PC Diagnostics";
/// Same bound as Windows' `CRED_MAX_CREDENTIAL_BLOB_SIZE`.
const MAX_SECRET: usize = 5 * 512;

/// `errSecItemNotFound`.
const ERR_ITEM_NOT_FOUND: i32 = -25_300;
/// `errSecUserCanceled`, `errSecAuthFailed`, `errSecInteractionNotAllowed`,
/// `errSecNoAccessForItem`: the user or the Keychain refused access.
const ACCESS_DENIED: [i32; 4] = [-128, -25_293, -25_308, -25_243];

fn platform_error(op: &str, e: SecError) -> AppError {
    let code = if ACCESS_DENIED.contains(&e.code()) {
        ErrorCode::PermissionDenied
    } else {
        ErrorCode::Platform
    };
    AppError::new(code, format!("macOS Keychain: {op} failed"))
        .with_detail(format!("OSStatus {}: {e}", e.code()))
}

/// Creates or replaces the generic password `service` with `secret`.
pub fn write(service: &str, secret: &[u8]) -> Result<(), AppError> {
    if secret.len() > MAX_SECRET {
        return Err(AppError::invalid_input("the credential is too large"));
    }
    passwords::set_generic_password(service, ACCOUNT, secret)
        .map_err(|e| platform_error("write", e))
}

/// The secret bytes, `None` when no item exists. The caller zeroes them after use.
pub fn read(service: &str) -> Result<Option<Vec<u8>>, AppError> {
    match passwords::get_generic_password(service, ACCOUNT) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.code() == ERR_ITEM_NOT_FOUND => Ok(None),
        Err(e) => Err(platform_error("read", e)),
    }
}

/// Deletes the item. Succeeds when there was none.
pub fn delete(service: &str) -> Result<(), AppError> {
    match passwords::delete_generic_password(service, ACCOUNT) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERR_ITEM_NOT_FOUND => Ok(()),
        Err(e) => Err(platform_error("delete", e)),
    }
}
