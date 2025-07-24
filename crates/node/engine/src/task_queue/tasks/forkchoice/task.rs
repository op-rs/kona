//! A task for the `engine_forkchoiceUpdated` method, with no attributes.

use crate::{
    EngineClient, EngineForkchoiceVersion, EngineState, EngineTaskExt, ForkchoiceTaskError,
    Metrics, state::EngineSyncStateUpdate,
};
use alloy_provider::ext::EngineApi;
use alloy_rpc_types_engine::{INVALID_FORK_CHOICE_STATE_ERROR, PayloadId, PayloadStatusEnum};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::OpAttributesWithParent;
use op_alloy_provider::ext::engine::OpEngineApi;
use std::sync::Arc;
use tokio::time::Instant;

/// The [`ForkchoiceTask`] executes an `engine_forkchoiceUpdated` call with the current
/// [`EngineState`]'s forkchoice, and no payload attributes.
#[derive(Debug, Clone)]
pub struct ForkchoiceTask {
    /// The engine client.
    pub client: Arc<EngineClient>,
    /// The rollup config.
    pub rollup: Arc<RollupConfig>,
    /// Optional payload attributes to be used for the forkchoice update.
    pub envelope: Option<OpAttributesWithParent>,
    /// The sync state update to apply to the engine state.
    pub state_update: EngineSyncStateUpdate,
}

impl ForkchoiceTask {
    /// Creates a new [`ForkchoiceTask`].
    pub const fn new(
        client: Arc<EngineClient>,
        rollup: Arc<RollupConfig>,
        state_update: EngineSyncStateUpdate,
        payload_attributes: Option<OpAttributesWithParent>,
    ) -> Self {
        Self { client, rollup, envelope: payload_attributes, state_update }
    }

    /// Checks the response of the `engine_forkchoiceUpdated` call, and updates the sync status if
    /// necessary.
    fn check_forkchoice_updated_status(
        &self,
        state: &mut EngineState,
        status: &PayloadStatusEnum,
    ) -> Result<(), ForkchoiceTaskError> {
        match status {
            PayloadStatusEnum::Valid => {
                if !state.el_sync_finished {
                    info!(
                        target: "engine",
                        "Finished execution layer sync."
                    );
                    state.el_sync_finished = true;
                }

                Ok(())
            }
            PayloadStatusEnum::Syncing => {
                if self.envelope.is_some() {
                    // If we're building a new payload, we should retry the FCU once the engine is
                    // done syncing.
                    debug!(target: "engine", "Build initiation FCU failed temporarily: EL is syncing");
                    Err(ForkchoiceTaskError::EngineSyncing)
                } else {
                    // If we're not building a new payload, we're driving EL sync.
                    Ok(())
                }
            }
            PayloadStatusEnum::Invalid { validation_error } => {
                error!(target: "engine", "Forkchoice update failed: {}", validation_error);
                Err(ForkchoiceTaskError::InvalidPayloadStatus(validation_error.clone()))
            }
            s => {
                // Other codes are never returned by `engine_forkchoiceUpdate`
                Err(ForkchoiceTaskError::UnexpectedPayloadStatus(s.clone()))
            }
        }
    }
}

#[async_trait]
impl EngineTaskExt for ForkchoiceTask {
    type Output = Option<PayloadId>;
    type Error = ForkchoiceTaskError;

    async fn execute(&self, state: &mut EngineState) -> Result<Self::Output, ForkchoiceTaskError> {
        // Apply the sync state update to the engine state.
        let new_sync_state = state.sync_state.apply_update(self.state_update);

        // Check if a forkchoice update is not needed, return early.
        // A forkchoice update is not needed if...
        // 1. The engine state is not default (initial forkchoice state has been emitted), and
        // 2. The new sync state is the same as the current sync state (no changes to the sync
        //    state).
        if state.sync_state != Default::default() &&
            state.sync_state == new_sync_state &&
            self.envelope.is_none()
        {
            return Err(ForkchoiceTaskError::NoForkchoiceUpdateNeeded);
        }

        // Check if the head is behind the finalized head.
        if new_sync_state.unsafe_head().block_info.number <
            new_sync_state.finalized_head().block_info.number
        {
            return Err(ForkchoiceTaskError::FinalizedAheadOfUnsafe(
                new_sync_state.unsafe_head().block_info.number,
                new_sync_state.finalized_head().block_info.number,
            ));
        }

        let fcu_time_start = Instant::now();

        // Determine the forkchoice version to use.
        // Note that if the envelope is not provided, we use the forkchoice version from the
        // timestamp zero. The version number in `fork_choice_updated_v*`
        // methods only matters for the payload attributes.
        let version = EngineForkchoiceVersion::from_cfg(
            &self.rollup,
            self.envelope.as_ref().map(|p| p.inner.payload_attributes.timestamp).unwrap_or(0),
        );

        // TODO(@theochap, `<https://github.com/op-rs/kona/issues/2387>`): we should avoid cloning the payload attributes here.
        let payload_attributes = self.envelope.as_ref().map(|p| p.inner()).cloned();

        // Send the forkchoice update through the input.
        let forkchoice = new_sync_state.create_forkchoice_state();

        // Handle the forkchoice update result.
        let response = match version {
            EngineForkchoiceVersion::V1 => {
                self.client
                    .fork_choice_updated_v1(
                        forkchoice,
                        payload_attributes.map(|p| p.payload_attributes),
                    )
                    .await
            }
            EngineForkchoiceVersion::V2 => {
                self.client.fork_choice_updated_v2(forkchoice, payload_attributes).await
            }
            EngineForkchoiceVersion::V3 => {
                self.client.fork_choice_updated_v3(forkchoice, payload_attributes).await
            }
        };

        let valid_response = response.map_err(|e| {
            // Fatal forkchoice update error.
            e.as_error_resp()
                .and_then(|e| {
                    (e.code == INVALID_FORK_CHOICE_STATE_ERROR as i64)
                        .then_some(ForkchoiceTaskError::InvalidForkchoiceState)
                })
                .unwrap_or_else(|| ForkchoiceTaskError::ForkchoiceUpdateFailed(e))
        })?;

        // Unexpected forkchoice payload status.
        // We may be able to recover from this by resetting the engine.
        self.check_forkchoice_updated_status(state, &valid_response.payload_status.status)?;

        // Apply the new sync state to the engine state.
        state.sync_state = new_sync_state;

        // Update metrics.
        kona_macros::inc!(counter, Metrics::ENGINE_TASK_COUNT, Metrics::FORKCHOICE_TASK_LABEL);

        let fcu_duration = fcu_time_start.elapsed();
        debug!(
            target: "engine",
            fcu_duration = ?fcu_duration,
            forkchoice = ?forkchoice,
            response = ?valid_response,
            "Forkchoice updated"
        );

        Ok(valid_response.payload_id)
    }
}
