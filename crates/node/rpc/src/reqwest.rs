//! RPC API implementation using `reqwest`

use alloy_rpc_client::ReqwestClient;
use derive_more::Constructor;
use kona_interop::{ExecutingMessage, SafetyLevel};

use crate::{CheckMessages, ExecutingMessageValidatorError};

/// A supervisor client.
#[derive(Debug, Clone, Constructor)]
pub struct SupervisorClient {
    /// The inner RPC client.
    client: ReqwestClient,
}

impl CheckMessages for SupervisorClient {
    async fn check_messages(
        &self,
        messages: &[ExecutingMessage],
        min_safety: SafetyLevel,
    ) -> Result<(), ExecutingMessageValidatorError> {
        self.client
            .request("supervisor_checkMessages", (messages, min_safety))
            .await
            .map_err(ExecutingMessageValidatorError::client)
    }
}
