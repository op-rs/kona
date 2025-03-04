//! Defines the supervisor API and Client.

use alloc::boxed::Box;
use alloy_primitives::Log;
use alloy_sol_types::SolEvent;
use core::{error, time::Duration};
use kona_interop::{
    CROSS_L2_INBOX_ADDRESS, ExecutingMessage, InvalidExecutingMessage, SafetyLevel,
};

/// Failures occurring during validation of [`ExecutingMessage`]s.
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

/// Subset of `op-supervisor` API, used for validating interop events.
///
/// <https://github.com/ethereum-optimism/optimism/blob/develop/op-supervisor/supervisor/frontend/frontend.go#L18-L28>
pub trait CheckMessages {
    /// Returns if the messages meet the minimum safety level.
    fn check_messages(
        &self,
        messages: &[ExecutingMessage],
        min_safety: SafetyLevel,
    ) -> impl Future<Output = Result<(), ExecutingMessageValidatorError>> + Send;
}

/// Interacts with a Supervisor to validate [`ExecutingMessage`]s.
#[async_trait::async_trait]
pub trait ExecutingMessageValidator {
    /// The supervisor client type.
    type SupervisorClient: CheckMessages + Send + Sync;

    /// Default duration that message validation is not allowed to exceed.
    const DEFAULT_TIMEOUT: Duration;

    /// Returns reference to supervisor client. Used in default trait method implementations.
    fn supervisor_client(&self) -> &Self::SupervisorClient;

    /// Extracts [`ExecutingMessage`]s from the [`Log`] if there are any.
    fn parse_messages(logs: &[Log]) -> impl Iterator<Item = Option<ExecutingMessage>> {
        logs.iter().map(|log| {
            (log.address == CROSS_L2_INBOX_ADDRESS && log.topics().len() == 2)
                .then(|| ExecutingMessage::decode_log_data(&log.data, true).ok())
                .flatten()
        })
    }

    /// Validates a list of [`ExecutingMessage`]s against a Supervisor.
    async fn validate_messages(
        &self,
        messages: &[ExecutingMessage],
        safety: SafetyLevel,
        timeout: Option<Duration>,
    ) -> Result<(), ExecutingMessageValidatorError> {
        // Set timeout duration based on input if provided.
        let timeout = timeout.unwrap_or(Self::DEFAULT_TIMEOUT);

        // Validate messages against supervisor with timeout.
        tokio::time::timeout(timeout, self.supervisor_client().check_messages(messages, safety))
            .await
            .map_err(|_| ExecutingMessageValidatorError::ValidationTimeout(timeout.as_secs()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpsee::types::ErrorObjectOwned;

    const MIN_SAFETY_ERROR: &str = r#"{"code":-32000,"message":"message {0x4200000000000000000000000000000000000023 4 1 1728507701 901} (safety level: unsafe) does not meet the minimum safety cross-unsafe"}"#;
    const INVALID_CHAIN: &str = r#"{"code":-32000,"message":"failed to check message: failed to check log: unknown chain: 14417"}"#;
    const INVALID_LEVEL: &str = r#"{"code":-32000,"message":"message {0x4200000000000000000000000000000000000023 1091637521 4369 0 901} (safety level: invalid) does not meet the minimum safety unsafe"}"#;
    const RANDOM_ERROR: &str = r#"{"code":-32000,"message":"gibberish error"}"#;

    #[test]
    #[cfg(feature = "jsonrpsee")]
    fn test_jsonrpsee_client_error_parsing() {
        assert!(matches!(
            ExecutingMessageValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(MIN_SAFETY_ERROR).unwrap()
            ),
            ExecutingMessageValidatorError::MinimumSafety {
                expected: SafetyLevel::CrossUnsafe,
                got: SafetyLevel::Unsafe
            }
        ));

        assert!(matches!(
            ExecutingMessageValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(INVALID_CHAIN).unwrap()
            ),
            ExecutingMessageValidatorError::UnknownChain(14417)
        ));

        assert!(matches!(
            ExecutingMessageValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(INVALID_LEVEL).unwrap()
            ),
            ExecutingMessageValidatorError::MinimumSafety {
                expected: SafetyLevel::Unsafe,
                got: SafetyLevel::Invalid
            }
        ));

        assert!(matches!(
            ExecutingMessageValidatorError::from(
                serde_json::from_str::<ErrorObjectOwned>(RANDOM_ERROR).unwrap()
            ),
            ExecutingMessageValidatorError::SupervisorServerError(_)
        ));
    }
}
