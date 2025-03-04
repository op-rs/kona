//! Interop Traits

use crate::SupervisorApiClient;
use alloc::boxed::Box;
use alloy_primitives::Log;
use alloy_sol_types::SolEvent;
use async_trait::async_trait;
use core::time::Duration;
use jsonrpsee::{core::ClientError, types::ErrorObjectOwned};
use kona_interop::{CROSS_L2_INBOX_ADDRESS, ExecutingMessage, SafetyLevel};
use tokio::time::error::Elapsed;

const UNKNOWN_CHAIN_MSG: &str = "unknown chain: ";
const MINIMUM_SAFETY_MSG: &str = "does not meet the minimum safety";
/// Failures occurring during validation of [`ExecutingMessage`]s.
#[derive(thiserror::Error, Debug)]
pub enum ExecutingMessageValidatorError {
    /// Message does not minimum safety level
    #[error("message does not meet safety level. expected: {expected}, Got: {got}")]
    MinimumSafety {
        /// Actual level of the message
        got: SafetyLevel,
        /// Level that was passed to supervisor
        expected: SafetyLevel,
    },
    /// Invalid chain
    #[error("unsupported chain id: {0}")]
    UnknownChain(u64),
    /// Failure from the [`SupervisorApiClient`] when validating messages.
    #[error("supervisor determined messages are invalid: {0}")]
    SupervisorRpcError(#[from] ClientError),

    /// Message validation against the Supervisor took longer than allowed.
    #[error("message validation timed out: {0}")]
    ValidationTimeout(#[from] Elapsed),

    /// Catch-all variant for other supervisor errors.
    #[error("unexpected error from supervisor: {0}")]
    Other(#[from] ErrorObjectOwned),
}

/// Interacts with a Supervisor to validate [`ExecutingMessage`]s.
#[async_trait]
pub trait ExecutingMessageValidator {
    /// The supervisor client type.
    type SupervisorClient: SupervisorApiClient + Send + Sync;

    /// Default duration that message validation is not allowed to exceed.
    const DEFAULT_TIMEOUT: Duration;

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
        supervisor: &Self::SupervisorClient,
        messages: &[ExecutingMessage],
        safety: SafetyLevel,
        timeout: Option<Duration>,
    ) -> Result<(), ExecutingMessageValidatorError> {
        // Set timeout duration based on input if provided.
        let timeout = timeout.map_or(Self::DEFAULT_TIMEOUT, |t| t);

        // Construct the future to validate all messages using supervisor.
        let fut = async { supervisor.check_messages(messages.to_vec(), safety).await };

        // Await the validation future with timeout.
        match tokio::time::timeout(timeout, fut)
            .await
            .map_err(ExecutingMessageValidatorError::ValidationTimeout)?
        {
            Ok(_) => Ok(()),
            Err(err) => {
                match err {
                    ClientError::Call(err) => {
                        // Check if it's invalid message call, message example:
                        // `failed to check message: failed to check log: unknown chain: 14417`
                        if err.message().contains(UNKNOWN_CHAIN_MSG) {
                            if let Ok(chain_id) = err
                                .message()
                                .split(' ')
                                .last()
                                .expect("message contains chain id")
                                .parse::<u64>()
                            {
                                return Err(ExecutingMessageValidatorError::UnknownChain(chain_id));
                            }
                        // Check if it's `does not meet the minimum safety` error, message example:
                        // `message {0x4200000000000000000000000000000000000023 4 1 1728507701 901}
                        // (safety level: unsafe) does not meet the minimum safety cross-unsafe"`
                        } else if err.message().contains(MINIMUM_SAFETY_MSG) {
                            let message_safety =
                                if err.message().contains("safety level: finalized") {
                                    SafetyLevel::Finalized
                                } else if err.message().contains("safety level: safe") {
                                    SafetyLevel::Safe
                                } else if err.message().contains("safety level: local-safe") {
                                    SafetyLevel::LocalSafe
                                } else if err.message().contains("safety level: cross-unsafe") {
                                    SafetyLevel::CrossUnsafe
                                } else if err.message().contains("safety level: unsafe") {
                                    SafetyLevel::Unsafe
                                } else if err.message().contains("safety level: invalid") {
                                    SafetyLevel::Invalid
                                } else {
                                    // Unexpected level name, return generic error
                                    return Err(ExecutingMessageValidatorError::Other(err));
                                };
                            return Err(ExecutingMessageValidatorError::MinimumSafety {
                                expected: safety,
                                got: message_safety,
                            });
                        }
                        Err(ExecutingMessageValidatorError::Other(err))
                    }
                    _ => Err(ExecutingMessageValidatorError::SupervisorRpcError(err)),
                }
            }
        }
    }
}
