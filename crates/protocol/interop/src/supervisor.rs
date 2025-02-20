//! Defines the supervisor API and Client.

use crate::{DerivedIdPair, ExecutingMessage, MessageIdentifier, SafetyLevel, SuperRootResponse};
use alloc::{boxed::Box, vec::Vec};
use alloy_eips::eip1898::BlockNumHash;
use alloy_primitives::B256;
use alloy_rpc_client::ReqwestClient;
use async_trait::async_trait;
use derive_more::{Display, From};
use kona_protocol::BlockInfo;

/// An interface for the `op-supervisor` component of the OP Stack.
///
/// <https://github.com/ethereum-optimism/optimism/blob/develop/op-supervisor/supervisor/frontend/frontend.go#L18-L28>
#[async_trait]
pub trait Supervisor {
    /// The error returned by supervisor methods.
    type Error: Send + Sync;

    /// Returns the safety level of a single message.
    async fn check_message(
        &self,
        identifier: MessageIdentifier,
        payload_hash: B256,
    ) -> Result<SafetyLevel, Self::Error>;

    /// Returns if the messages meet the minimum safety level.
    async fn check_messages(
        &self,
        messages: &[ExecutingMessage],
        min_safety: SafetyLevel,
    ) -> Result<(), Self::Error>;

    /// Returns the block at which the given block was derived from.
    async fn cross_derived_from(
        &self,
        chain_id: u32,
        derived: BlockNumHash,
    ) -> Result<BlockInfo, Self::Error>;

    /// Returns the local unsafe block.
    async fn local_unsafe(&self, chain_id: u32) -> Result<BlockNumHash, Self::Error>;

    /// Returns the cross safe block.
    async fn cross_safe(&self, chain_id: u32) -> Result<DerivedIdPair, Self::Error>;

    /// Returns the finalized block.
    async fn finalized(&self, chain_id: u32) -> Result<BlockNumHash, Self::Error>;

    /// Returns the finalized L1 block.
    async fn finalized_l1(&self) -> Result<BlockInfo, Self::Error>;

    /// Returns the super root at the given timestamp.
    async fn super_root_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<SuperRootResponse, Self::Error>;

    /// Returns all safe derived blocks at the given block.
    async fn all_safe_derived_at(
        &self,
        derived_from: BlockNumHash,
    ) -> Result<Vec<(u32, BlockNumHash)>, Self::Error>;
}

/// An error from the `op-supervisor`.
#[derive(Copy, Clone, Debug, Display, From, thiserror::Error)]
pub enum SupervisorError {
    /// A failed request.
    RequestFailed,
}

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

#[async_trait]
impl Supervisor for SupervisorClient {
    type Error = SupervisorError;

    async fn check_message(
        &self,
        identifier: MessageIdentifier,
        payload_hash: B256,
    ) -> Result<SafetyLevel, Self::Error> {
        self.client
            .request("supervisor_checkMessage", (identifier, payload_hash))
            .await
            .map_err(|_| SupervisorError::RequestFailed)
    }

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

    async fn cross_derived_from(&self, _: u32, _: BlockNumHash) -> Result<BlockInfo, Self::Error> {
        unimplemented!()
    }

    async fn local_unsafe(&self, _: u32) -> Result<BlockNumHash, Self::Error> {
        unimplemented!()
    }

    async fn cross_safe(&self, _: u32) -> Result<DerivedIdPair, Self::Error> {
        unimplemented!()
    }

    async fn finalized(&self, _: u32) -> Result<BlockNumHash, Self::Error> {
        unimplemented!()
    }

    async fn finalized_l1(&self) -> Result<BlockInfo, Self::Error> {
        unimplemented!()
    }

    async fn super_root_at_timestamp(&self, _: u64) -> Result<SuperRootResponse, Self::Error> {
        unimplemented!()
    }

    async fn all_safe_derived_at(
        &self,
        _: BlockNumHash,
    ) -> Result<Vec<(u32, BlockNumHash)>, Self::Error> {
        unimplemented!()
    }
}
