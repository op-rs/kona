//! A task for the `engine_forkchoiceUpdated` method, with no attributes.

use crate::{EngineClient, EngineState, EngineTaskError, EngineTaskExt, ForkchoiceTaskError};
use alloy_rpc_types_engine::INVALID_FORK_CHOICE_STATE_ERROR;
use async_trait::async_trait;
use op_alloy_provider::ext::engine::OpEngineApi;
use std::sync::Arc;

/// The [ForkchoiceTask] executes an `engine_forkchoiceUpdated` call with the current
/// [EngineState]'s forkchoice, and no payload attributes.
#[derive(Debug, Clone)]
pub struct ForkchoiceTask {
    /// The engine client.
    pub client: Arc<EngineClient>,
}

impl ForkchoiceTask {
    /// Creates a new [ForkchoiceTask].
    pub const fn new(client: Arc<EngineClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl EngineTaskExt for ForkchoiceTask {
    async fn execute(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        // Check if a forkchoice update is not needed, return early.
        if !state.forkchoice_update_needed {
            return Err(ForkchoiceTaskError::NoForkchoiceUpdateNeeded.into());
        }

        // If the engine is syncing, log a warning. We can still attempt to apply the forkchoice
        // update.
        if state.sync_status.is_syncing() {
            warn!(target: "engine", "Attempting to update forkchoice state while EL syncing");
        }

        // Check if the head is behind the finalized head.
        if state.unsafe_head().block_info.number < state.finalized_head().block_info.number {
            return Err(ForkchoiceTaskError::FinalizedAheadOfUnsafe(
                state.unsafe_head().block_info.number,
                state.finalized_head().block_info.number,
            )
            .into());
        }

        // Send the forkchoice update through the input.
        let forkchoice = state.create_forkchoice_state();

        // Handle the forkchoice update result.
        if let Err(e) = self.client.fork_choice_updated_v3(forkchoice, None).await {
            let e = e
                .as_error_resp()
                .and_then(|e| {
                    (e.code == INVALID_FORK_CHOICE_STATE_ERROR as i64)
                        .then_some(ForkchoiceTaskError::InvalidForkchoiceState)
                })
                .unwrap_or_else(|| ForkchoiceTaskError::ForkchoiceUpdateFailed(e));

            return Err(e.into());
        }

        state.forkchoice_update_needed = false;
        Ok(())
    }
}
