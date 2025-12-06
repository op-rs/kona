//! A task to insert an unsafe payload into the execution engine.

use crate::{
    EngineClient, EngineState, EngineTaskExt, InsertTaskError, SynchronizeTask,
    state::EngineSyncStateUpdate,
};
use alloy_eips::eip7685::EMPTY_REQUESTS_HASH;
use alloy_rpc_types_engine::{
    CancunPayloadFields, ExecutionPayloadInputV2, PayloadStatusEnum, PraguePayloadFields,
};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;
use op_alloy_consensus::OpBlock;
use op_alloy_rpc_types_engine::{
    OpExecutionPayload, OpExecutionPayloadEnvelope, OpExecutionPayloadSidecar,
};
use std::{sync::Arc, time::Instant};

/// The task to insert a payload into the execution engine.
#[derive(Debug, Clone)]
pub struct InsertTask<EngineClient_: EngineClient> {
    /// The engine client.
    client: Arc<EngineClient_>,
    /// The rollup config.
    rollup_config: Arc<RollupConfig>,
    /// The network payload envelope.
    envelope: OpExecutionPayloadEnvelope,
    /// If the payload is safe this is true.
    /// A payload is safe if it is derived from a safe block.
    is_payload_safe: bool,
}

impl<EngineClient_: EngineClient> InsertTask<EngineClient_> {
    /// Creates a new insert task.
    pub const fn new(
        client: Arc<EngineClient_>,
        rollup_config: Arc<RollupConfig>,
        envelope: OpExecutionPayloadEnvelope,
        is_attributes_derived: bool,
    ) -> Self {
        Self { client, rollup_config, envelope, is_payload_safe: is_attributes_derived }
    }

    /// Checks the response of the `engine_newPayload` call.
    const fn check_new_payload_status(&self, status: &PayloadStatusEnum) -> bool {
        matches!(status, PayloadStatusEnum::Valid | PayloadStatusEnum::Syncing)
    }
}

#[async_trait]
impl<EngineClient_: EngineClient> EngineTaskExt for InsertTask<EngineClient_> {
    type Output = ();

    type Error = InsertTaskError;

    async fn execute(&self, state: &mut EngineState) -> Result<(), InsertTaskError> {
        let time_start = Instant::now();

        // Insert the new payload.
        // Form the new unsafe block ref from the execution payload.
        let parent_beacon_block_root = self.envelope.parent_beacon_block_root.unwrap_or_default();
        let insert_time_start = Instant::now();
        let (response, block): (_, OpBlock) = match self.envelope.execution_payload.clone() {
            OpExecutionPayload::V1(payload) => (
                self.client.new_payload_v1(payload).await,
                self.envelope
                    .execution_payload
                    .clone()
                    .try_into_block()
                    .map_err(InsertTaskError::FromBlockError)?,
            ),
            OpExecutionPayload::V2(payload) => {
                let payload_input = ExecutionPayloadInputV2 {
                    execution_payload: payload.payload_inner,
                    withdrawals: Some(payload.withdrawals),
                };
                (
                    self.client.new_payload_v2(payload_input).await,
                    self.envelope
                        .execution_payload
                        .clone()
                        .try_into_block()
                        .map_err(InsertTaskError::FromBlockError)?,
                )
            }
            OpExecutionPayload::V3(payload) => (
                self.client.new_payload_v3(payload, parent_beacon_block_root).await,
                self.envelope
                    .execution_payload
                    .clone()
                    .try_into_block_with_sidecar(&OpExecutionPayloadSidecar::v3(
                        CancunPayloadFields::new(parent_beacon_block_root, vec![]),
                    ))
                    .map_err(InsertTaskError::FromBlockError)?,
            ),
            OpExecutionPayload::V4(payload) => (
                self.client.new_payload_v4(payload, parent_beacon_block_root).await,
                self.envelope
                    .execution_payload
                    .clone()
                    .try_into_block_with_sidecar(&OpExecutionPayloadSidecar::v4(
                        CancunPayloadFields::new(parent_beacon_block_root, vec![]),
                        PraguePayloadFields::new(EMPTY_REQUESTS_HASH),
                    ))
                    .map_err(InsertTaskError::FromBlockError)?,
            ),
        };

        // Check the `engine_newPayload` response.
        let response = match response {
            Ok(resp) => resp,
            Err(e) => {
                warn!(target: "engine", "Failed to insert new payload: {e}");
                return Err(InsertTaskError::InsertFailed(e));
            }
        };
        if !self.check_new_payload_status(&response.status) {
            return Err(InsertTaskError::UnexpectedPayloadStatus(response.status));
        }
        let insert_duration = insert_time_start.elapsed();

        let new_unsafe_ref =
            L2BlockInfo::from_block_and_genesis(&block, &self.rollup_config.genesis)
                .map_err(InsertTaskError::L2BlockInfoConstruction)?;

        // When inserting an unsafe payload, we need to maintain the invariant:
        // finalized <= safe <= local_safe <= cross_unsafe <= unsafe
        // If new_unsafe_ref is behind current safe heads (e.g., during reorg from P2P),
        // we must also update local_safe and safe to maintain the invariant.
        let new_block_number = new_unsafe_ref.block_info.number;

        // Update local_safe_head if payload is safe (derived) or if new block is behind current
        // local_safe (to maintain invariant during reorg)
        let local_safe_update = (self.is_payload_safe ||
            new_block_number < state.sync_state.local_safe_head().block_info.number)
            .then_some(new_unsafe_ref);

        // Update safe_head if payload is safe (derived) or if new block is behind current safe
        // (to maintain invariant during reorg)
        let safe_update = (self.is_payload_safe ||
            new_block_number < state.sync_state.safe_head().block_info.number)
            .then_some(new_unsafe_ref);

        // Send a FCU to canonicalize the imported block.
        SynchronizeTask::new(
            Arc::clone(&self.client),
            self.rollup_config.clone(),
            EngineSyncStateUpdate {
                cross_unsafe_head: Some(new_unsafe_ref),
                unsafe_head: Some(new_unsafe_ref),
                local_safe_head: local_safe_update,
                safe_head: safe_update,
                ..Default::default()
            },
        )
        .execute(state)
        .await?;

        let total_duration = time_start.elapsed();

        info!(
            target: "engine",
            hash = %new_unsafe_ref.block_info.hash,
            number = new_unsafe_ref.block_info.number,
            total_duration = ?total_duration,
            insert_duration = ?insert_duration,
            "Inserted new unsafe block"
        );

        Ok(())
    }
}
