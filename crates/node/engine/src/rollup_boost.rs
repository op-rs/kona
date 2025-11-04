//! Rollup-boost abstraction used by the engine client.

use alloy_primitives::{B256, Bytes};
use alloy_rpc_types_engine::{
    ExecutionPayloadV3, ForkchoiceState, ForkchoiceUpdated, PayloadId, PayloadStatus,
};
use op_alloy_rpc_types_engine::{
    OpExecutionPayloadEnvelopeV3, OpExecutionPayloadEnvelopeV4, OpExecutionPayloadV4,
    OpPayloadAttributes,
};
use rollup_boost::{EngineApiExt, EngineApiServer, RollupBoostServer};
use std::fmt::Debug;
use thiserror::Error;

/// Error wrapper for rollup-boost calls.
#[derive(Debug, Error)]
#[error("{0}")]
pub struct RollupBoostError(pub String);

/// Trait object used to erase the concrete rollup-boost server type.
#[async_trait::async_trait]
pub trait RollupBoostServerLike: Debug + Send + Sync {
    /// Creates a new payload v3.
    async fn new_payload_v3(
        &self,
        payload: ExecutionPayloadV3,
        versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
    ) -> Result<PayloadStatus, RollupBoostError>;

    /// Creates a new payload v4.
    async fn new_payload_v4(
        &self,
        payload: OpExecutionPayloadV4,
        versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
        execution_requests: Vec<Bytes>,
    ) -> Result<PayloadStatus, RollupBoostError>;

    /// Performs a fork choice updated v3.
    async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> Result<ForkchoiceUpdated, RollupBoostError>;

    /// Gets a payload v3.
    async fn get_payload_v3(
        &self,
        payload_id: PayloadId,
    ) -> Result<OpExecutionPayloadEnvelopeV3, RollupBoostError>;

    /// Gets a payload v4.
    async fn get_payload_v4(
        &self,
        payload_id: PayloadId,
    ) -> Result<OpExecutionPayloadEnvelopeV4, RollupBoostError>;
}

#[async_trait::async_trait]
impl<T: EngineApiExt + Send + Sync + 'static + Debug> RollupBoostServerLike
    for RollupBoostServer<T>
{
    async fn new_payload_v3(
        &self,
        payload: ExecutionPayloadV3,
        versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
    ) -> Result<PayloadStatus, RollupBoostError> {
        EngineApiServer::new_payload_v3(self, payload, versioned_hashes, parent_beacon_block_root)
            .await
            .map_err(|e| RollupBoostError(e.to_string()))
    }

    async fn new_payload_v4(
        &self,
        payload: OpExecutionPayloadV4,
        versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
        execution_requests: Vec<Bytes>,
    ) -> Result<PayloadStatus, RollupBoostError> {
        EngineApiServer::new_payload_v4(
            self,
            payload.clone(),
            versioned_hashes,
            parent_beacon_block_root,
            execution_requests,
        )
        .await
        .map_err(|e| RollupBoostError(e.to_string()))
    }

    async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> Result<ForkchoiceUpdated, RollupBoostError> {
        EngineApiServer::fork_choice_updated_v3(self, fork_choice_state, payload_attributes)
            .await
            .map_err(|e| RollupBoostError(e.to_string()))
    }

    async fn get_payload_v3(
        &self,
        payload_id: PayloadId,
    ) -> Result<OpExecutionPayloadEnvelopeV3, RollupBoostError> {
        EngineApiServer::get_payload_v3(self, payload_id)
            .await
            .map_err(|e| RollupBoostError(e.to_string()))
    }

    async fn get_payload_v4(
        &self,
        payload_id: PayloadId,
    ) -> Result<OpExecutionPayloadEnvelopeV4, RollupBoostError> {
        EngineApiServer::get_payload_v4(self, payload_id)
            .await
            .map_err(|e| RollupBoostError(e.to_string()))
    }
}
