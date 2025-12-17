//! Implements the rollup client rpc endpoints. These endpoints serve data about the rollup state.
//!
//! Implemented in the op-node in <https://github.com/ethereum-optimism/optimism/blob/174e55f0a1e73b49b80a561fd3fedd4fea5770c6/op-service/sources/rollupclient.go#L16>

use alloy_eips::BlockNumberOrTag;
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use kona_engine::EngineState;
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OutputRoot, SyncStatus};
use std::fmt::Debug;
use tokio::sync::watch;

use crate::{
    L1State, L1WatcherQueries, OutputResponse, RollupNodeApiServer, SafeHeadResponse,
    l1_watcher::L1WatcherQuerySender,
};

/// Trait to be referenced by those interacting with EngineActor for RPC
/// operations. The EngineActor requires the use of channels for communication, but
/// this interface allows that to be abstracted from callers and allows easy testing.
#[async_trait]
pub trait EngineRpcClient: Debug + Send + Sync + Clone {
    /// Request the current rollup configuration.
    async fn get_config(&self) -> RpcResult<RollupConfig>;
    /// Request the current [`EngineState`] snapshot.
    async fn get_state(&self) -> RpcResult<EngineState>;
    /// Development API: Get the current number of pending tasks in the queue.
    async fn get_task_queue_length(&self) -> RpcResult<usize>;
    /// Request the L2 output root for a specific block.
    ///
    /// Returns a tuple of block info, output root, and engine state at the requested block.
    async fn output_at_block(
        &self,
        block: BlockNumberOrTag,
    ) -> RpcResult<(L2BlockInfo, OutputRoot, EngineState)>;
    /// Subscribes to engine queue length updates managed by the returned [`watch::Receiver`].
    async fn dev_subscribe_to_engine_queue_length(&self) -> RpcResult<watch::Receiver<usize>>;
    /// Subscribes to engine state updates managed by the returned [`watch::Receiver`].
    async fn dev_subscribe_to_engine_state(&self) -> RpcResult<watch::Receiver<EngineState>>;
}

/// RollupRpc
///
/// This is a server implementation of [`crate::RollupNodeApiServer`].
#[derive(Debug)]
pub struct RollupRpc<EngineRpcClient_> {
    /// The channel to send [`kona_engine::EngineQueries`]s.
    pub engine_client: EngineRpcClient_,
    /// The channel to send [`crate::L1WatcherQueries`]s.
    pub l1_watcher_sender: L1WatcherQuerySender,
}

impl<EngineRpcClient_: EngineRpcClient> RollupRpc<EngineRpcClient_> {
    /// The identifier for the Metric that tracks rollup RPC calls.
    pub const RPC_IDENT: &'static str = "rollup_rpc";

    /// Constructs a new [`RollupRpc`] given a sender channel.
    pub const fn new(
        engine_client: EngineRpcClient_,
        l1_watcher_sender: L1WatcherQuerySender,
    ) -> Self {
        Self { engine_client, l1_watcher_sender }
    }

    // Important note: we zero-out the fields that can't be derived yet to follow op-node's
    // behaviour.
    fn sync_status_from_actor_queries(
        l1_sync_status: L1State,
        l2_sync_status: EngineState,
    ) -> SyncStatus {
        SyncStatus {
            current_l1: l1_sync_status.current_l1.unwrap_or_default(),
            current_l1_finalized: l1_sync_status.current_l1_finalized.unwrap_or_default(),
            head_l1: l1_sync_status.head_l1.unwrap_or_default(),
            safe_l1: l1_sync_status.safe_l1.unwrap_or_default(),
            finalized_l1: l1_sync_status.finalized_l1.unwrap_or_default(),
            unsafe_l2: l2_sync_status.sync_state.unsafe_head(),
            cross_unsafe_l2: l2_sync_status.sync_state.cross_unsafe_head(),
            local_safe_l2: l2_sync_status.sync_state.local_safe_head(),
            safe_l2: l2_sync_status.sync_state.safe_head(),
            finalized_l2: l2_sync_status.sync_state.finalized_head(),
        }
    }
}

#[async_trait]
impl<EngineRpcClient_: EngineRpcClient + 'static> RollupNodeApiServer
    for RollupRpc<EngineRpcClient_>
{
    async fn op_output_at_block(&self, block_num: BlockNumberOrTag) -> RpcResult<OutputResponse> {
        kona_macros::inc!(gauge, Self::RPC_IDENT, "method" => "op_outputAtBlock");

        let (l1_sync_status_send, l1_sync_status_recv) = tokio::sync::oneshot::channel();

        let ((l2_block_info, output_root, l2_sync_status), l1_sync_status) = tokio::try_join!(
            async { self.engine_client.output_at_block(block_num).await },
            async {
                self.l1_watcher_sender
                    .send(L1WatcherQueries::L1State(l1_sync_status_send))
                    .await
                    .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

                l1_sync_status_recv.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
            }
        )?;

        let sync_status = Self::sync_status_from_actor_queries(l1_sync_status, l2_sync_status);

        Ok(OutputResponse::from_v0(output_root, sync_status, l2_block_info))
    }

    /// This RPC endpoint is not supported. It is not necessary to track the safe head for every L1
    /// block post-interop anymore so we can remove this method from the rpc interface.
    async fn op_safe_head_at_l1_block(
        &self,
        _block_num: BlockNumberOrTag,
    ) -> RpcResult<SafeHeadResponse> {
        kona_macros::inc!(gauge, Self::RPC_IDENT, "method" => "op_safeHeadAtL1Block");
        return Err(ErrorObject::from(ErrorCode::MethodNotFound));
    }

    async fn op_sync_status(&self) -> RpcResult<SyncStatus> {
        kona_macros::inc!(gauge, Self::RPC_IDENT, "method" => "op_syncStatus");

        let (l1_sync_status_send, l1_sync_status_recv) = tokio::sync::oneshot::channel();

        let (l1_sync_status, l2_sync_status) = tokio::try_join!(
            async {
                self.l1_watcher_sender
                    .send(L1WatcherQueries::L1State(l1_sync_status_send))
                    .await
                    .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;
                l1_sync_status_recv.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
            },
            async { self.engine_client.get_state().await }
        )
        .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        return Ok(Self::sync_status_from_actor_queries(l1_sync_status, l2_sync_status));
    }

    async fn op_rollup_config(&self) -> RpcResult<RollupConfig> {
        kona_macros::inc!(gauge, Self::RPC_IDENT, "method" => "op_rollupConfig");

        self.engine_client.get_config().await
    }

    async fn op_version(&self) -> RpcResult<String> {
        kona_macros::inc!(gauge, Self::RPC_IDENT, "method" => "op_version");

        const RPC_VERSION: &str = env!("CARGO_PKG_VERSION");

        return Ok(RPC_VERSION.to_string());
    }
}
