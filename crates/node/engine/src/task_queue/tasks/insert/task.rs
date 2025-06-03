//! A task to insert an unsafe payload into the execution engine.

use crate::{
    EngineClient, EngineForkchoiceVersion, EngineState, EngineTaskError, EngineTaskExt,
    InsertUnsafeTaskError, Metrics,
};
use alloy_eips::eip7685::EMPTY_REQUESTS_HASH;
use alloy_provider::ext::EngineApi;
use alloy_rpc_types_engine::{
    CancunPayloadFields, ExecutionPayloadInputV2, ForkchoiceState, INVALID_FORK_CHOICE_STATE_ERROR,
    PayloadStatusEnum, PraguePayloadFields,
};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;
use op_alloy_consensus::OpBlock;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::{
    OpExecutionPayload, OpExecutionPayloadSidecar, OpNetworkPayloadEnvelope,
};
use std::{sync::Arc, time::Instant};

/// The task to insert an unsafe payload into the execution engine.
#[derive(Debug, Clone)]
pub struct InsertUnsafeTask {
    /// The engine client.
    client: Arc<EngineClient>,
    /// The rollup config.
    rollup_config: Arc<RollupConfig>,
    /// The engine forkchoice version.
    version: EngineForkchoiceVersion,
    /// The network payload envelope.
    envelope: OpNetworkPayloadEnvelope,
}

impl InsertUnsafeTask {
    /// Creates a new insert task.
    pub fn new(
        client: Arc<EngineClient>,
        rollup_config: Arc<RollupConfig>,
        envelope: OpNetworkPayloadEnvelope,
    ) -> Self {
        let version =
            EngineForkchoiceVersion::from_cfg(rollup_config.as_ref(), envelope.payload.timestamp());
        Self { client, rollup_config, version, envelope }
    }

    /// Checks the response of the `engine_newPayload` call.
    const fn check_new_payload_status(&self, status: &PayloadStatusEnum) -> bool {
        matches!(status, PayloadStatusEnum::Valid | PayloadStatusEnum::Syncing)
    }

    /// Checks the response of the `engine_forkchoiceUpdated` call, and updates the sync status if
    /// necessary.
    fn check_forkchoice_updated_status(
        &self,
        state: &mut EngineState,
        status: &PayloadStatusEnum,
    ) -> bool {
        if matches!(status, PayloadStatusEnum::Valid) && !state.el_sync_finished {
            info!(
                target: "engine",
                "Finished execution layer sync."
            );
            state.el_sync_finished = true;
        }
        matches!(status, PayloadStatusEnum::Valid | PayloadStatusEnum::Syncing)
    }
}

#[async_trait]
impl EngineTaskExt for InsertUnsafeTask {
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

        let fcu = ForkchoiceState {
            head_block_hash: new_unsafe_ref.block_info.hash,
            safe_block_hash: state.safe_head().block_info.hash,
            finalized_block_hash: state.finalized_head().block_info.hash,
        };

        // Send the forkchoice update to finalize the payload insertion.
        let fcu_time_start = Instant::now();
        let response = match self.version {
            EngineForkchoiceVersion::V1 => self.client.fork_choice_updated_v1(fcu, None).await,
            EngineForkchoiceVersion::V2 => self.client.fork_choice_updated_v2(fcu, None).await,
            EngineForkchoiceVersion::V3 => self.client.fork_choice_updated_v3(fcu, None).await,
        };
        let fcu_duration = fcu_time_start.elapsed();
        let total_duration = time_start.elapsed();

        let response = match response {
            Ok(resp) => resp,
            Err(e) => {
                // Check if the error is due to an inconsistent forkchoice state. If so, we need to
                // signal for a pipeline reset.
                let e = e
                    .as_error_resp()
                    .and_then(|err| {
                        (err.code == INVALID_FORK_CHOICE_STATE_ERROR as i64)
                            .then_some(InsertUnsafeTaskError::InconsistentForkchoiceState)
                    })
                    .unwrap_or(InsertUnsafeTaskError::ForkchoiceUpdateFailed(e));
                return Err(e.into());
            }
        };
        if !self.check_forkchoice_updated_status(state, &response.payload_status.status) {
            return Err(InsertUnsafeTaskError::UnexpectedPayloadStatus(
                response.payload_status.status,
            )
            .into());
        }

        // Update the local engine state.
        state.set_cross_unsafe_head(new_unsafe_ref);
        state.set_unsafe_head(new_unsafe_ref);
        state.forkchoice_update_needed = false;

        info!(
            target: "engine",
            hash = %new_unsafe_ref.block_info.hash,
            number = new_unsafe_ref.block_info.number,
            total_duration = ?total_duration,
            insert_duration = ?insert_duration,
            fcu_duration = ?fcu_duration,
            "Inserted new unsafe block"
        );

        // Update metrics.
        kona_macros::inc!(counter, Metrics::ENGINE_TASK_COUNT, Metrics::INSERT_TASK_LABEL);

        Ok(())
    }
}
