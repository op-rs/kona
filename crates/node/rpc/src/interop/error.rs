//! Interop supervisor RPC client errors.

use core::error;
use kona_interop::InvalidInboxEntry;

/// Failures occurring during validation of inbox entries.
#[derive(thiserror::Error, Debug)]
pub enum InteropTxValidatorError {
    /// Error validating interop event.
    #[error(transparent)]
    InvalidInboxEntry(#[from] InvalidInboxEntry),

    /// RPC client failure.
    #[error("supervisor rpc client failure: {0}")]
    RpcClientError(Box<dyn error::Error + Send + Sync>),

    /// Message validation against the Supervisor took longer than allowed.
    #[error("message validation timed out, timeout: {0} secs")]
    ValidationTimeout(u64),

    /// Catch-all variant for other supervisor server errors.
    #[error("unexpected error from supervisor: {0}")]
    SupervisorServerError(Box<dyn error::Error + Send + Sync>),
}

impl InteropTxValidatorError {
    /// Returns a new instance of [`RpcClientError`](Self::RpcClientError) variant.
    pub fn client(err: impl error::Error + Send + Sync + 'static) -> Self {
        Self::RpcClientError(Box::new(err))
    }

    /// Returns a new instance of [`RpcClientError`](Self::RpcClientError) variant.
    pub fn server_unexpected(err: impl error::Error + Send + Sync + 'static) -> Self {
        Self::SupervisorServerError(Box::new(err))
    }
}

#[cfg(feature = "client")]
impl From<jsonrpsee::core::ClientError> for InteropTxValidatorError {
    fn from(err: jsonrpsee::core::ClientError) -> Self {
        match err {
            jsonrpsee::core::ClientError::Call(err) => err.into(),
            _ => Self::client(err),
        }
    }
}

impl From<jsonrpsee::types::ErrorObjectOwned> for InteropTxValidatorError {
    fn from(err: jsonrpsee::types::ErrorObjectOwned) -> Self {
        InvalidInboxEntry::parse_err_msg(err.message())
            .map(Self::InvalidInboxEntry)
            .unwrap_or(Self::server_unexpected(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpsee::types::ErrorObjectOwned;
    use kona_interop::{InvalidInboxEntry, SafetyLevel};

    const MIN_SAFETY_LOCAL_SAFE_ERROR: &str = r#"{"code":-32000,"message":"message {0x4200000000000000000000000000000000000023 4 1 1728507701 901} (safety level: cross-unsafe) does not meet the minimum safety local-safe"}"#;
    const MIN_SAFETY_SAFE_ERROR: &str = r#"{"code":-32000,"message":"message {0x4200000000000000000000000000000000000023 1091637521 4369 0 901} (safety level: local-safe) does not meet the minimum safety safe"}"#;
    const INVALID_CHAIN: &str = r#"{"code":-32000,"message":"failed to check message: failed to check log: unknown chain: 14417"}"#;
    const RANDOM_VALIDATION_ERROR: &str = r#"{"code":-32000,"message":"gibberish error"}"#;

    #[test]
    fn test_jsonrpsee_client_error_parsing() {
        assert!(matches!(
            InteropTxValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(MIN_SAFETY_LOCAL_SAFE_ERROR).unwrap()
            ),
            InteropTxValidatorError::InvalidInboxEntry(InvalidInboxEntry::MinimumSafety {
                expected: SafetyLevel::LocalSafe,
                got: SafetyLevel::CrossUnsafe
            })
        ));

        assert!(matches!(
            InteropTxValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(MIN_SAFETY_SAFE_ERROR).unwrap()
            ),
            InteropTxValidatorError::InvalidInboxEntry(InvalidInboxEntry::MinimumSafety {
                expected: SafetyLevel::Safe,
                got: SafetyLevel::LocalSafe
            })
        ));

        assert!(matches!(
            InteropTxValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(INVALID_CHAIN).unwrap()
            ),
            InteropTxValidatorError::InvalidInboxEntry(InvalidInboxEntry::UnknownChain(14417))
        ));

        assert!(matches!(
            InteropTxValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(RANDOM_VALIDATION_ERROR).unwrap()
            ),
            InteropTxValidatorError::SupervisorServerError(_)
        ));

        #[cfg(feature = "client")]
        assert!(matches!(
            InteropTxValidatorError::from(jsonrpsee::core::ClientError::RequestTimeout),
            InteropTxValidatorError::RpcClientError(_)
        ));
    }
}
