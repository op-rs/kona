//! RPC API implementation using `reqwest`

use alloy_primitives::B256;
use alloy_rpc_client::ReqwestClient;
use derive_more::Constructor;
use kona_interop::{ExecutingDescriptor, SafetyLevel};

use crate::{CheckAccessList, InteropTxValidatorError};

/// A supervisor client.
#[derive(Debug, Clone, Constructor)]
pub struct SupervisorClient {
    /// The inner RPC client.
    client: ReqwestClient,
}

impl CheckAccessList for SupervisorClient {
    async fn check_access_list(
        &self,
        inbox_entries: &[B256],
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), InteropTxValidatorError> {
        self.client
            .request(
                "supervisor_checkAccessList",
                (inbox_entries, min_safety, executing_descriptor),
            )
            .await
            .map_err(InteropTxValidatorError::client)
    }
}
