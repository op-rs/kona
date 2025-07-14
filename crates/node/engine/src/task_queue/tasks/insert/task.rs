//! A task to insert an unsafe payload into the execution engine.

use crate::{
    EngineClient, EngineState, EngineTaskError, EngineTaskExt, ForkchoiceTask,
    InsertUnsafeTaskError, Metrics, state::EngineSyncStateUpdate,
};
use alloy_eips::eip7685::EMPTY_REQUESTS_HASH;
use alloy_provider::ext::EngineApi;
use alloy_rpc_types_engine::{
    CancunPayloadFields, ExecutionPayloadInputV2, PayloadStatusEnum, PraguePayloadFields,
};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;
use op_alloy_consensus::OpBlock;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::{
    OpExecutionPayload, OpExecutionPayloadEnvelope, OpExecutionPayloadSidecar,
};
use std::{sync::Arc, time::Instant};

/// The task to insert an unsafe payload into the execution engine.
#[derive(Debug, Clone)]
pub struct InsertUnsafeTask {
    /// The engine client.
    client: Arc<EngineClient>,
    /// The rollup config.
    rollup_config: Arc<RollupConfig>,
    /// The network payload envelope.
    envelope: OpExecutionPayloadEnvelope,
}

impl InsertUnsafeTask {
    /// Creates a new insert task.
    pub const fn new(
        client: Arc<EngineClient>,
        rollup_config: Arc<RollupConfig>,
        envelope: OpExecutionPayloadEnvelope,
    ) -> Self {
        Self { client, rollup_config, envelope }
    }

    /// Checks the response of the `engine_newPayload` call.
    const fn check_new_payload_status(&self, status: &PayloadStatusEnum) -> bool {
        matches!(status, PayloadStatusEnum::Valid | PayloadStatusEnum::Syncing)
    }
}

#[async_trait]
impl EngineTaskExt for InsertUnsafeTask {
    type Output = ();

    async fn execute(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        let time_start = Instant::now();

        // Insert the new payload.
        // Form the new unsafe block ref from the execution payload.
        let parent_beacon_block_root = self.envelope.parent_beacon_block_root.unwrap_or_default();
        let insert_time_start = Instant::now();
        let (response, block): (_, OpBlock) = match self.envelope.payload.clone() {
            OpExecutionPayload::V1(payload) => (
                self.client.new_payload_v1(payload).await,
                self.envelope
                    .payload
                    .clone()
                    .try_into_block()
                    .map_err(InsertUnsafeTaskError::FromBlockError)?,
            ),
            OpExecutionPayload::V2(payload) => {
                let payload_input = ExecutionPayloadInputV2 {
                    execution_payload: payload.payload_inner,
                    withdrawals: Some(payload.withdrawals),
                };
                (
                    self.client.new_payload_v2(payload_input).await,
                    self.envelope
                        .payload
                        .clone()
                        .try_into_block()
                        .map_err(InsertUnsafeTaskError::FromBlockError)?,
                )
            }
            OpExecutionPayload::V3(payload) => (
                self.client.new_payload_v3(payload, parent_beacon_block_root).await,
                self.envelope
                    .payload
                    .clone()
                    .try_into_block_with_sidecar(&OpExecutionPayloadSidecar::v3(
                        CancunPayloadFields::new(parent_beacon_block_root, vec![]),
                    ))
                    .map_err(InsertUnsafeTaskError::FromBlockError)?,
            ),
            OpExecutionPayload::V4(payload) => (
                self.client.new_payload_v4(payload, parent_beacon_block_root).await,
                self.envelope
                    .payload
                    .clone()
                    .try_into_block_with_sidecar(&OpExecutionPayloadSidecar::v4(
                        CancunPayloadFields::new(parent_beacon_block_root, vec![]),
                        PraguePayloadFields::new(EMPTY_REQUESTS_HASH),
                    ))
                    .map_err(InsertUnsafeTaskError::FromBlockError)?,
            ),
        };

        // Check the `engine_newPayload` response.
        let response = match response {
            Ok(resp) => resp,
            Err(e) => return Err(InsertUnsafeTaskError::InsertFailed(e).into()),
        };
        if !self.check_new_payload_status(&response.status) {
            return Err(InsertUnsafeTaskError::UnexpectedPayloadStatus(response.status).into());
        }
        let insert_duration = insert_time_start.elapsed();

        let new_unsafe_ref =
            L2BlockInfo::from_block_and_genesis(&block, &self.rollup_config.genesis)
                .map_err(InsertUnsafeTaskError::L2BlockInfoConstruction)?;

        // Send a FCU to canonicalize the imported block.
        ForkchoiceTask::new(
            Arc::clone(&self.client),
            self.rollup_config.clone(),
            EngineSyncStateUpdate {
                cross_unsafe_head: Some(new_unsafe_ref),
                unsafe_head: Some(new_unsafe_ref),
                ..Default::default()
            },
            None,
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

        // Update metrics.
        kona_macros::inc!(counter, Metrics::ENGINE_TASK_COUNT, Metrics::INSERT_TASK_LABEL);

        Ok(())
    }
}
