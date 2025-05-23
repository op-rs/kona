//! Interop RPC API.

use alloy_primitives::B256;
use kona_interop::{ExecutingDescriptor, SafetyLevel};
use kona_supervisor_rpc::SupervisorApiClient;

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
    ) -> impl Future<Output = Result<(), InteropTxValidatorError>> + Send;
}

impl<T> CheckAccessListClient for T
where
    T: SupervisorApiClient + Send + Sync,
{
    // codecov:ignore-start
    async fn check_access_list(
        &self,
        inbox_entries: &[B256],
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), InteropTxValidatorError> {
        Ok(T::check_access_list(self, inbox_entries.to_vec(), min_safety, executing_descriptor)
            .await?)
    }
    // codecov:ignore-end
}
