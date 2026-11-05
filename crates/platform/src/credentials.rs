//! The user's Anthropic API key, stored in Windows Credential Manager.
//!
//! One generic credential ([`API_KEY_TARGET`], `CRED_PERSIST_LOCAL_MACHINE`: this user on
//! this PC, not roamed). Windows protects the blob with the user's logon secrets (DPAPI);
//! the app never writes the key to disk, settings or logs.
//!
//! - [`store_api_key`]: validates the format, then creates or replaces the credential.
//! - [`load_api_key`]: the key, or `None` if none is stored.
//! - [`delete_api_key`]: removes it (`Ok` if there was none).
//! - [`has_api_key`]: whether one is stored (for `AssistantStatus::key_configured`).
//!
//! Intermediate buffers are zeroed after use (best effort: the returned `String` belongs to
//! the caller). The key is never included in errors or log events. On other platforms every
//! call fails with `NotSupported` (and [`has_api_key`] is `false`).

use models::{AppError, ErrorCode};

/// Credential Manager target name ("Credenziali generiche" in the Control Panel).
pub const API_KEY_TARGET: &str = "PCDiagnostics/AnthropicApiKey";
/// Longest accepted key, in characters (Anthropic keys are ~110).
pub const MAX_API_KEY_LEN: usize = 512;

/// Stores (or replaces) the API key. Leading/trailing whitespace is trimmed; the key must
/// then be non-empty, at most [`MAX_API_KEY_LEN`] characters of printable ASCII without
/// spaces, else `InvalidInput`.
pub fn store_api_key(key: &str) -> Result<(), AppError> {
    store_at(API_KEY_TARGET, key)
}

/// The stored API key, `None` if there is none.
pub fn load_api_key() -> Result<Option<String>, AppError> {
    load_at(API_KEY_TARGET)
}

/// Deletes the stored API key. Succeeds when there was none.
pub fn delete_api_key() -> Result<(), AppError> {
    delete_at(API_KEY_TARGET)
}

/// `true` if an API key is stored and readable. Errors count as "no key".
pub fn has_api_key() -> bool {
    matches!(load_at(API_KEY_TARGET), Ok(Some(_)))
}

/// Format check shared by [`store_api_key`]. Returns the trimmed key. Never echoes the key.
pub fn validate_api_key(key: &str) -> Result<&str, AppError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::invalid_input("the API key is empty"));
    }
    if key.len() > MAX_API_KEY_LEN {
        return Err(AppError::invalid_input(format!(
            "the API key is too long (at most {MAX_API_KEY_LEN} characters)"
        )));
    }
    if !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(AppError::invalid_input(
            "the API key contains invalid characters (only printable ASCII characters, no spaces)",
        ));
    }
    Ok(key)
}

/// Overwrites a buffer with zeros in a way the optimizer may not elide.
pub(crate) fn zeroize(buf: &mut [u8]) {
    buf.fill(0);
    std::hint::black_box(&mut *buf);
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

fn store_at(target: &str, key: &str) -> Result<(), AppError> {
    let key = validate_api_key(key)?;
    #[cfg(windows)]
    {
        crate::windows::credentials::write(target, key.as_bytes())
    }
    #[cfg(not(windows))]
    {
        let _ = (target, key);
        Err(not_supported())
    }
}

fn load_at(target: &str) -> Result<Option<String>, AppError> {
    #[cfg(windows)]
    {
        let Some(bytes) = crate::windows::credentials::read(target)? else {
            return Ok(None);
        };
        match String::from_utf8(bytes) {
            Ok(s) => Ok(Some(s)),
            Err(e) => {
                let mut bytes = e.into_bytes();
                zeroize(&mut bytes);
                Err(AppError::new(
                    ErrorCode::Platform,
                    "the stored API key is corrupted: enter it again",
                ))
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = target;
        Err(not_supported())
    }
}

fn delete_at(target: &str) -> Result<(), AppError> {
    #[cfg(windows)]
    {
        crate::windows::credentials::delete(target)
    }
    #[cfg(not(windows))]
    {
        let _ = target;
        Err(not_supported())
    }
}

#[cfg(not(windows))]
fn not_supported() -> AppError {
    AppError::new(
        ErrorCode::NotSupported,
        "the credential store is only available on Windows",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_check() {
        assert_eq!(
            validate_api_key("  sk-ant-api03-abc_DEF-123\n").expect("valid"),
            "sk-ant-api03-abc_DEF-123"
        );
        for bad in ["", "   ", "sk ant", "sk-ant-é", "sk-\u{7f}", "a\tb"] {
            let e = validate_api_key(bad).expect_err(bad);
            assert_eq!(e.code, ErrorCode::InvalidInput);
            assert!(!e.message.contains(bad.trim()) || bad.trim().is_empty());
        }
        assert!(validate_api_key(&"k".repeat(MAX_API_KEY_LEN)).is_ok());
        assert!(validate_api_key(&"k".repeat(MAX_API_KEY_LEN + 1)).is_err());
    }

    #[test]
    fn zeroize_clears() {
        let mut b = b"secret".to_vec();
        zeroize(&mut b);
        assert!(b.iter().all(|&x| x == 0));
    }

    #[test]
    fn invalid_key_is_rejected_before_storage() {
        let e = store_at("PCDiagnostics/Test/never-written", "bad key").expect_err("rejected");
        assert_eq!(e.code, ErrorCode::InvalidInput);
    }

    /// Real Credential Manager round trip on a test-only target (the user's real key is
    /// never touched).
    #[cfg(windows)]
    #[test]
    fn credential_manager_round_trip() {
        let target = format!("PCDiagnostics/Test/ApiKey-{}", std::process::id());
        let _ = delete_at(&target);
        assert_eq!(load_at(&target).expect("load absent"), None);
        delete_at(&target).expect("deleting an absent key is ok");

        store_at(&target, " sk-ant-test-0123456789 ").expect("store");
        assert_eq!(
            load_at(&target).expect("load").as_deref(),
            Some("sk-ant-test-0123456789")
        );
        store_at(&target, "sk-ant-test-replaced").expect("replace");
        assert_eq!(
            load_at(&target).expect("load").as_deref(),
            Some("sk-ant-test-replaced")
        );
        delete_at(&target).expect("delete");
        assert_eq!(load_at(&target).expect("load deleted"), None);
    }
}
