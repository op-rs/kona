//! Defines the supervisor API and Client.

use crate::{ExecutingMessage, SafetyLevel};
use alloc::boxed::Box;
use async_trait::async_trait;

/// An interface for the `op-supervisor` component of the OP Stack.
///
/// <https://github.com/ethereum-optimism/optimism/blob/develop/op-supervisor/supervisor/frontend/frontend.go#L18-L28>
#[async_trait]
pub trait CheckMessages {
    /// The error returned by supervisor methods.
    type Error: Send + Sync;

    /// Returns if the messages meet the minimum safety level.
    async fn check_messages(
        &self,
        messages: &[ExecutingMessage],
        min_safety: SafetyLevel,
    ) -> Result<(), Self::Error>;
}
