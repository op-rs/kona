//! RPC validator component used in interop.
//!
//! [`InteropTxValidator`] parses inbox entries from [`AccessListItem`]s, and queries a
//! superchain supervisor for their validity via RPC using the [`CheckAccessList`] API.

mod api;
pub use api::CheckAccessList;

mod error;
pub use error::InteropTxValidatorError;

use alloc::boxed::Box;
use alloy_eips::eip2930::AccessListItem;
use alloy_primitives::B256;
use core::time::Duration;
use kona_interop::{ExecutingDescriptor, SafetyLevel, parse_access_list_items_to_inbox_entries};

/// Interacts with a Supervisor to validate inbox entries extracted from [`AccessListItem`]s.
#[async_trait::async_trait]
pub trait InteropTxValidator {
    /// The supervisor client type.
    type SupervisorClient: CheckAccessList + Send + Sync;

    /// Default duration that message validation is not allowed to exceed.
    const DEFAULT_TIMEOUT: Duration;

    /// Returns reference to supervisor client. Used in default trait method implementations.
    fn supervisor_client(&self) -> &Self::SupervisorClient;

    /// Extracts inbox entries from the [`AccessListItem`]s if there are any.
    fn parse_access_list(access_list_items: &[AccessListItem]) -> impl Iterator<Item = &B256> {
        parse_access_list_items_to_inbox_entries(access_list_items.iter())
    }

    /// Validates a list of inbox entries against a Supervisor.
    async fn validate_messages(
        &self,
        inbox_entries: &[B256],
        safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
        timeout: Option<Duration>,
    ) -> Result<(), InteropTxValidatorError> {
        // Set timeout duration based on input if provided.
        let timeout = timeout.unwrap_or(Self::DEFAULT_TIMEOUT);

        // Validate messages against supervisor with timeout.
        tokio::time::timeout(
            timeout,
            self.supervisor_client().check_access_list(inbox_entries, safety, executing_descriptor),
        )
        .await
        .map_err(|_| InteropTxValidatorError::ValidationTimeout(timeout.as_secs()))?
    }
}
