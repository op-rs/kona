//! Implements the rollup client rpc endpoints. These endpoints serve data about the rollup state.
//!
//! Implemented in the op-node in <https://github.com/ethereum-optimism/optimism/blob/174e55f0a1e73b49b80a561fd3fedd4fea5770c6/op-service/sources/rollupclient.go#L16>

use alloy_eips::BlockNumberOrTag;
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use kona_genesis::RollupConfig;
use kona_protocol::SyncStatus;
use tokio::sync::oneshot::Sender;

use crate::{NetworkRpc, OutputResponse, RollupNodeApiServer, SafeHeadResponse};

/// The supported rollup RPC requests.
///
/// <https://docs.optimism.io/builders/node-operators/json-rpc>
/// <https://github.com/ethereum-optimism/optimism/blob/8dd17a7b114a7c25505cd2e15ce4e3d0f7e3f7c1/op-node/node/api.go#L114>
#[derive(Debug)]
pub enum RollupRpcRequest {
    /// Provides information about the output state at a given block.
    OutputAtBlock {
        /// The block number to query.
        block: BlockNumberOrTag,
        /// A sender to gather the output response.
        handler: Sender<OutputResponse>,
    },
    /// Provides information about the safe head at a given block.
    SafeHeadAtL1Block {
        /// The block number to query.
        block: BlockNumberOrTag,
        /// A sender to gather the safe head response.
        handler: Sender<SafeHeadResponse>,
    },
    /// Provides information about the sync status of the rollup node.
    SyncStatus(Sender<SyncStatus>),
    /// Provides information about the current rollup config.
    RollupConfig(Sender<RollupConfig>),
    /// Provides information about the rollup node version.
    Version(Sender<String>),
}

impl RollupRpcRequest {
    /// Handles the rollup rpc request.
    pub fn handle(self) {
        unimplemented!("Rollup RPC request handling is not implemented yet")
    }
}

#[async_trait]
impl RollupNodeApiServer for NetworkRpc {
    async fn op_output_at_block(&self, block_num: BlockNumberOrTag) -> RpcResult<OutputResponse> {
        // TODO(@theochap): add metrics

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rollup_sender
            .send(RollupRpcRequest::OutputAtBlock { block: block_num, handler: tx })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn op_safe_head_at_l1_block(
        &self,
        block_num: BlockNumberOrTag,
    ) -> RpcResult<SafeHeadResponse> {
        // TODO(@theochap): add metrics

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rollup_sender
            .send(RollupRpcRequest::SafeHeadAtL1Block { block: block_num, handler: tx })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn op_sync_status(&self) -> RpcResult<SyncStatus> {
        // TODO(@theochap): add metrics

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rollup_sender
            .send(RollupRpcRequest::SyncStatus(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn op_rollup_config(&self) -> RpcResult<RollupConfig> {
        // TODO(@theochap): add metrics

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rollup_sender
            .send(RollupRpcRequest::RollupConfig(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn op_version(&self) -> RpcResult<String> {
        // TODO(@theochap): add metrics

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rollup_sender
            .send(RollupRpcRequest::Version(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }
}
