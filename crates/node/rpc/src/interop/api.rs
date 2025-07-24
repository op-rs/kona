//! Interop RPC API.

use alloy_primitives::B256;
use kona_interop::{ExecutingDescriptor, SafetyLevel};

use crate::InteropTxValidatorError;

/// Subset of `op-supervisor` API, used for validating interop events.
// TODO: add link once https://github.com/ethereum-optimism/optimism/pull/14784 merged
pub trait CheckAccessListClient {
    /// Returns if the messages meet the minimum safety level.
    fn check_access_list(
        &self,
        inbox_entries: &[B256],
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> impl std::future::Future<Output = Result<(), InteropTxValidatorError>> + Send;
}
