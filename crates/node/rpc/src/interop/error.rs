//! Interop supervisor RPC client errors.

use core::error;
use op_alloy_rpc_types::SuperchainDAError;

/// Failures occurring during validation of inbox entries.
#[derive(thiserror::Error, Debug)]
pub enum InteropTxValidatorError {
    /// Error validating interop event.
    #[error(transparent)]
    InvalidInboxEntry(#[from] SuperchainDAError),

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
        SuperchainDAError::try_from(err.code())
            .map(Self::InvalidInboxEntry)
            .unwrap_or(Self::server_unexpected(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpsee::types::ErrorObjectOwned;

    const RANDOM_VALIDATION_ERROR: &str = r#"{"code":-32000,"message":"gibberish error"}"#;

    #[test]
    fn test_jsonrpsee_client_error_parsing() {
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
