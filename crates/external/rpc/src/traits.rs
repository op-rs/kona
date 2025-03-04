//! Interop Traits

use crate::SupervisorApiClient;
use alloc::boxed::Box;
use alloy_primitives::Log;
use alloy_sol_types::SolEvent;
use async_trait::async_trait;
use core::time::Duration;
use jsonrpsee::core::ClientError;
use kona_interop::{CROSS_L2_INBOX_ADDRESS, CheckMessages, ExecutingMessage, SafetyLevel};
use tokio::time::error::Elapsed;

/// Failures occurring during validation of [`ExecutingMessage`]s.
#[derive(thiserror::Error, Debug)]
pub enum ExecutingMessageValidatorError {
    /// Failure from the [`SupervisorApiClient`] when validating messages.
    #[error("Supervisor determined messages are invalid: {0}")]
    SupervisorRpcError(#[from] ClientError),

    /// Message validation against the Supervisor took longer than allowed.
    #[error("Message validation timed out: {0}")]
    ValidationTimeout(#[from] Elapsed),
}

/// Interacts with a Supervisor to validate [`ExecutingMessage`]s.
#[async_trait]
pub trait ExecutingMessageValidator {
    /// The supervisor client type.
    type SupervisorClient: CheckMessages + Send + Sync + Clone;

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

        // Construct the future to validate all messages using supervisor.
        let client = self.supervisor_client().clone();
        let fut = async {
            client
                .check_messages(messages.to_vec(), safety)
                .await
                .map_err(ExecutingMessageValidatorError::SupervisorRpcError)
        };

        // Await the validation future with timeout.
        tokio::time::timeout(timeout, fut)
            .await
            .map_err(ExecutingMessageValidatorError::ValidationTimeout)?
    }
}
