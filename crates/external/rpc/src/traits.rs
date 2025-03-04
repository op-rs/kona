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

const SAFETY_START_MSG: &str = "(safety level: ";
const UNKNOWN_CHAIN_MSG: &str = "unknown chain: ";
const MINIMUM_SAFETY_MSG: &str = "does not meet the minimum safety";
/// Failures occurring during validation of [`ExecutingMessage`]s.
#[derive(thiserror::Error, Debug)]
pub enum ExecutingMessageValidatorError {
    /// Message does not minimum safety level
    #[error("Message does not meet safety level. Expected: {0}, Got: {1}")]
    MinimumSafety(SafetyLevel, SafetyLevel),
    /// Invalid chain
    #[error("Unsupported chainId: {0}")]
    UnknownChain(u64),
    /// Failure from the [`SupervisorApiClient`] when validating messages.
    #[error("Supervisor determined messages are invalid: {0}")]
    SupervisorRpcError(#[from] ClientError),

    /// Message validation against the Supervisor took longer than allowed.
    #[error("Message validation timed out: {0}")]
    ValidationTimeout(#[from] Elapsed),

    /// Catch-all variant for other supervisor errors.
    #[error("Unexpected error from supervisor: {0}")]
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
        let res = tokio::time::timeout(timeout, fut)
            .await
            .map_err(ExecutingMessageValidatorError::ValidationTimeout)?;
        match res {
            Ok(_) => Ok(()),
            Err(err) => {
                match err {
                    ClientError::Call(err) => {
                        // Check if it's invalid message call, message example:
                        // `failed to check message: failed to check log: unknown chain: 14417`
                        if let Some(msg_start) = err.message().find(UNKNOWN_CHAIN_MSG) {
                            let chain_id_start = msg_start + UNKNOWN_CHAIN_MSG.len();
                            if let Ok(chain_id) = &err.message()[chain_id_start..].parse::<u64>() {
                                return Err(ExecutingMessageValidatorError::UnknownChain(*chain_id));
                            }
                        // Check if it's `does not meet the minimum safety` error, message example:
                        // `message {0x4200000000000000000000000000000000000023 4 1 1728507701 901}
                        // (safety level: unsafe) does not meet the minimum safety cross-unsafe"`
                        } else if err.message().contains(MINIMUM_SAFETY_MSG) {
                            if let Some(msg_start) = err.message().find(SAFETY_START_MSG) {
                                let safety_start = msg_start + SAFETY_START_MSG.len();
                                if let Some(safety_end) = &err.message()[safety_start..].find(")") {
                                    if let Ok(got_safety_level) = serde_json::from_str(&&err.message()
                                        [safety_start..safety_start + safety_end])
                                    {
                                        return Err(ExecutingMessageValidatorError::MinimumSafety(
                                            safety,
                                            got_safety_level,
                                        ));
                                    }
                                }
                            }
                        }
                        Err(ExecutingMessageValidatorError::Other(err))
                    }
                    _ => Err(ExecutingMessageValidatorError::SupervisorRpcError(err)),
                }
            }
        }
    }
}
