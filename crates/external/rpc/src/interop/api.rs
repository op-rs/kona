//! Interop RPC API.

use kona_interop::{ExecutingMessage, SafetyLevel};

use crate::ExecutingMessageValidatorError;

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
