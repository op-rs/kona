//! Supervisor client implementation.

use crate::{
    DerivedIdPair, ExecutingMessage, MessageIdentifier, SafetyLevel, SuperRootResponse, Supervisor,
};
use alloc::{boxed::Box, vec::Vec};
use alloy_eips::eip1898::BlockNumHash;
use alloy_primitives::B256;
use alloy_rpc_client::ReqwestClient;
use async_trait::async_trait;
use derive_more::{Display, From};
use kona_protocol::BlockInfo;

/// A supervisor client.
#[derive(Debug, Clone)]
pub struct SupervisorClient {
    /// The inner RPC client.
    client: ReqwestClient,
}

impl SupervisorClient {
    /// Creates a new `SupervisorClient` with the given `client`.
    pub const fn new(client: ReqwestClient) -> Self {
        Self { client }
    }
}

/// An error from the `op-supervisor`.
#[derive(Copy, Clone, Debug, Display, From, thiserror::Error)]
pub enum SupervisorError {
    /// A failed request.
    RequestFailed,
}

#[async_trait]
impl CheckMessages for SupervisorClient {
    type Error = SupervisorError;

    async fn check_messages(
        &self,
        messages: &[ExecutingMessage],
        min_safety: SafetyLevel,
    ) -> Result<(), Self::Error> {
        self.client
            .request("supervisor_checkMessages", (messages, min_safety))
            .await
            .map_err(|_| SupervisorError::RequestFailed)
    }
}
