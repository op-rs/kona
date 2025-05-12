use std::sync::Arc;

use crate::{EngineClient, EngineState};
use kona_genesis::RollupConfig;
use kona_sources::{SyncStartError, find_starting_forkchoice};

use super::{EngineTaskError, EngineTaskExt, ForkchoiceTask};

/// The [RestartTask] restarts the engine state by finding the starting forkchoice state using the
/// [`find_starting_forkchoice`] method. Once the forkchoice state is found, a new FCU is sent to
/// the engine.
#[derive(Debug, Clone)]
pub struct RestartTask {
    /// The engine client.
    pub client: Arc<EngineClient>,
    /// The [RollupConfig].
    pub cfg: Arc<RollupConfig>,
}

impl RestartTask {
    /// Executes the [`ForkchoiceTask`] if the attributes match the block.
    pub async fn execute_forkchoice_task(
        &self,
        state: &mut EngineState,
    ) -> Result<(), EngineTaskError> {
        let task = ForkchoiceTask::new(Arc::clone(&self.client));
        task.execute(state).await
    }
}

/// An error that can occur during the [RestartTask].
#[derive(Debug, thiserror::Error)]
pub enum RestartTaskError {
    /// Failed to update the forkchoice.
    #[error(transparent)]
    ForkchoiceUpdateFailed(#[from] EngineTaskError),
    /// Failed to find a starting forkchoice state.
    #[error("Failed to find a starting forkchoice state. Error {0}")]
    SyncStartError(#[from] SyncStartError),
}

impl From<RestartTaskError> for EngineTaskError {
    fn from(value: RestartTaskError) -> Self {
        match value {
            RestartTaskError::ForkchoiceUpdateFailed(e) => Self::Temporary(Box::new(e)),
            RestartTaskError::SyncStartError(e) => Self::Critical(Box::new(e)),
        }
    }
}

#[async_trait::async_trait]
impl EngineTaskExt for RestartTask {
    async fn execute(&self, state: &mut crate::EngineState) -> Result<(), crate::EngineTaskError> {
        info!("Restarting engine state");

        let (mut l1_provider, mut l2_provider) = self.client.as_alloy_chain_providers();
        let l2_forkchoice = find_starting_forkchoice(&self.cfg, &mut l1_provider, &mut l2_provider)
            .await
            .map_err(Into::<RestartTaskError>::into)?;

        state.set_unsafe_head(l2_forkchoice.un_safe);
        state.set_safe_head(l2_forkchoice.safe);
        state.set_local_safe_head(l2_forkchoice.safe);
        state.set_finalized_head(l2_forkchoice.finalized);

        state.forkchoice_update_needed = true;
        self.execute_forkchoice_task(state).await?;
        Ok(())
    }
}
