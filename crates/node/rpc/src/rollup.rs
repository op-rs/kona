//! Implements the rollup client rpc endpoints. These endpoints serve data about the rollup state.
//!
//! Implemented in the op-node in <https://github.com/ethereum-optimism/optimism/blob/174e55f0a1e73b49b80a561fd3fedd4fea5770c6/op-service/sources/rollupclient.go#L16>

use alloy_eips::BlockNumberOrTag;
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use kona_engine::EngineQuerySender;
use kona_genesis::RollupConfig;
use kona_protocol::SyncStatus;

use crate::{OutputResponse, RollupNodeApiServer, SafeHeadResponse};

/// RollupRpc
///
/// This is a server implementation of [`crate::RollupNodeApiServer`].
#[derive(Debug)]
pub struct RollupRpc {
    /// The channel to send [`kona_engine::EngineStateQuery`]s.
    pub engine_sender: EngineQuerySender,
    // TODO(@theochap): Add channels to send requests to the derivation actor and the
    // l1 watcher.
}

impl RollupRpc {
    /// Constructs a new [`RollupRpc`] given a sender channel.
    pub const fn new(sender: EngineQuerySender) -> Self {
        Self { engine_sender: sender }
    }
}

#[async_trait]
impl RollupNodeApiServer for RollupRpc {
    async fn op_output_at_block(&self, _block_num: BlockNumberOrTag) -> RpcResult<OutputResponse> {
        // TODO(@theochap): add metrics
        // TODO(@theochap): implement this RPC endpoint.
        return Err(ErrorObject::from(ErrorCode::MethodNotFound));
    }

    async fn op_safe_head_at_l1_block(
        &self,
        _block_num: BlockNumberOrTag,
    ) -> RpcResult<SafeHeadResponse> {
        // TODO(@theochap): add metrics
        // TODO(@theochap): implement this RPC endpoint.
        return Err(ErrorObject::from(ErrorCode::MethodNotFound));
    }

    async fn op_sync_status(&self) -> RpcResult<SyncStatus> {
        // TODO(@theochap): add metrics
        // TODO(@theochap): implement this RPC endpoint.
        return Err(ErrorObject::from(ErrorCode::MethodNotFound));
    }

    async fn op_rollup_config(&self) -> RpcResult<RollupConfig> {
        // TODO(@theochap): add metrics
        // TODO(@theochap): implement this RPC endpoint.
        return Err(ErrorObject::from(ErrorCode::MethodNotFound));
    }

    async fn op_version(&self) -> RpcResult<String> {
        // TODO(@theochap): add metrics
        // TODO(@theochap): implement this RPC endpoint.
        return Err(ErrorObject::from(ErrorCode::MethodNotFound));
    }
}
