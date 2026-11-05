use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Machine-readable error category. The UI picks wording and icon from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ErrorCode {
    InvalidInput,
    NotFound,
    PermissionDenied,
    NotSupported,
    Platform,
    Database,
    /// Another exclusive operation (e.g. a heavy benchmark) is already running.
    Busy,
    Cancelled,
    /// The action needs a user opt-in that is off (e.g. online checks).
    ConsentRequired,
    /// An external service (e.g. the AI assistant API) failed or could not be reached.
    Network,
    Internal,
}

/// Error returned by every command.
///
/// `message` is a precise English description; the UI derives localized copy from
/// `code` and shows `message`/`detail` in a technical disclosure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[error("{code:?}: {message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<String>,
    pub retriable: bool,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
            retriable: matches!(
                code,
                ErrorCode::Busy | ErrorCode::Platform | ErrorCode::Database | ErrorCode::Network
            ),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camel_case_fields_and_code() {
        let err = AppError::invalid_input("bad interval").with_detail("got 10");
        let json = serde_json::to_value(&err).expect("serialize");
        assert_eq!(json["code"], "invalidInput");
        assert_eq!(json["message"], "bad interval");
        assert_eq!(json["detail"], "got 10");
        assert_eq!(json["retriable"], false);
    }

    #[test]
    fn transient_codes_are_retriable() {
        assert!(AppError::new(ErrorCode::Busy, "x").retriable);
        assert!(!AppError::new(ErrorCode::PermissionDenied, "x").retriable);
    }
}
