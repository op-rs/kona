//! RPC validator component used in interop.
//!
//! [`ExecutingMessageValidator`] parses interop events from [`Log`]s, and queries a
//! superchain supervisor for their validity via RPC using the [`CheckMessages`] API.

mod api;
pub use api::CheckMessages;

mod error;
pub use error::ExecutingMessageValidatorError;

use alloc::boxed::Box;
use alloy_primitives::Log;
use core::time::Duration;
use kona_interop::{ExecutingMessage, SafetyLevel, parse_logs_to_executing_msgs};

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
        parse_logs_to_executing_msgs(logs.iter())
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
