//! Interop supervisor RPC client errors.

use alloc::boxed::Box;
use core::error;
use kona_interop::InvalidExecutingMessage;

/// Failures occurring during validation of [`ExecutingMessage`](kona_interop::ExecutingMessage)s.
#[derive(thiserror::Error, Debug)]
pub enum ExecutingMessageValidatorError {
    /// Error validating interop event.
    #[error(transparent)]
    InvalidExecutingMessage(#[from] InvalidExecutingMessage),

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

impl ExecutingMessageValidatorError {
    /// Returns a new instance of [`RpcClientError`](Self::RpcClientError) variant.
    pub fn client(err: impl error::Error + Send + Sync + 'static) -> Self {
        Self::RpcClientError(Box::new(err))
    }

    /// Returns a new instance of [`RpcClientError`](Self::RpcClientError) variant.
    pub fn server_unexpected(err: impl error::Error + Send + Sync + 'static) -> Self {
        Self::SupervisorServerError(Box::new(err))
    }
}

#[cfg(feature = "jsonrpsee")]
impl From<jsonrpsee::core::ClientError> for ExecutingMessageValidatorError {
    fn from(err: jsonrpsee::core::ClientError) -> Self {
        match err {
            jsonrpsee::core::ClientError::Call(err) => err.into(),
            _ => Self::client(err),
        }
    }
}

#[cfg(feature = "jsonrpsee")]
impl From<jsonrpsee::types::ErrorObjectOwned> for ExecutingMessageValidatorError {
    fn from(err: jsonrpsee::types::ErrorObjectOwned) -> Self {
        InvalidExecutingMessage::parse_err_msg(err.message())
            .map(Self::InvalidExecutingMessage)
            .unwrap_or(Self::server_unexpected(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpsee::{core::ClientError, types::ErrorObjectOwned};
    use kona_interop::{InvalidExecutingMessage, SafetyLevel};

    const MIN_SAFETY_LOCAL_SAFE_ERROR: &str = r#"{"code":-32000,"message":"message {0x4200000000000000000000000000000000000023 4 1 1728507701 901} (safety level: cross-unsafe) does not meet the minimum safety local-safe"}"#;
    const MIN_SAFETY_SAFE_ERROR: &str = r#"{"code":-32000,"message":"message {0x4200000000000000000000000000000000000023 1091637521 4369 0 901} (safety level: local-safe) does not meet the minimum safety safe"}"#;
    const INVALID_CHAIN: &str = r#"{"code":-32000,"message":"failed to check message: failed to check log: unknown chain: 14417"}"#;
    const RANDOM_VALIDATION_ERROR: &str = r#"{"code":-32000,"message":"gibberish error"}"#;

    #[test]
    #[cfg(feature = "jsonrpsee")]
    fn test_jsonrpsee_client_error_parsing() {
        assert!(matches!(
            ExecutingMessageValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(MIN_SAFETY_LOCAL_SAFE_ERROR).unwrap()
            ),
            ExecutingMessageValidatorError::InvalidExecutingMessage(
                InvalidExecutingMessage::MinimumSafety {
                    expected: SafetyLevel::LocalSafe,
                    got: SafetyLevel::CrossUnsafe
                }
            )
        ));

        assert!(matches!(
            ExecutingMessageValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(MIN_SAFETY_SAFE_ERROR).unwrap()
            ),
            ExecutingMessageValidatorError::InvalidExecutingMessage(
                InvalidExecutingMessage::MinimumSafety {
                    expected: SafetyLevel::Safe,
                    got: SafetyLevel::LocalSafe
                }
            )
        ));

        assert!(matches!(
            ExecutingMessageValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(INVALID_CHAIN).unwrap()
            ),
            ExecutingMessageValidatorError::InvalidExecutingMessage(
                InvalidExecutingMessage::UnknownChain(14417)
            )
        ));

        assert!(matches!(
            ExecutingMessageValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(RANDOM_VALIDATION_ERROR).unwrap()
            ),
            ExecutingMessageValidatorError::SupervisorServerError(_)
        ));

        assert!(matches!(
            ExecutingMessageValidatorError::from(ClientError::RequestTimeout),
            ExecutingMessageValidatorError::RpcClientError(_)
        ));
    }
}
