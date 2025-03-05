//! A task for the `engine_forkchoiceUpdated` query.

use crate::{
    EngineForkchoiceVersion, EngineState, EngineTask, ForkchoiceTaskError, ForkchoiceTaskInput,
    ForkchoiceTaskOut, SyncStatus,
};

/// The receiver type for the forkchoice task.
type Receiver = tokio::sync::broadcast::Receiver<ForkchoiceTaskInput>;

/// The sender type for the forkchoice task.
type Sender = tokio::sync::broadcast::Sender<ForkchoiceTaskOut>;

/// The task to update the forkchoice.
#[derive(Debug)]
pub struct ForkchoiceTask {
    /// A receiver channel to receive messages from an external actor.
    pub receiver: Receiver,
    /// A sender channel to send messages out to an external actor.
    pub sender: Sender,
}

impl ForkchoiceTask {
    /// Creates a new forkchoice task.
    pub const fn new(receiver: Receiver, sender: Sender) -> Self {
        Self { receiver, sender }
    }

    /// Fetches a state snapshot through the external API.
    pub async fn fetch_state(&mut self) -> Result<EngineState, ForkchoiceTaskError> {
        self.sender
            .send(ForkchoiceTaskOut::StateSnapshot)
            .map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        let response = self.receiver.recv().await.map_err(|_| ForkchoiceTaskError::ReceiveError)?;
        if let ForkchoiceTaskInput::StateResponse(response) = response {
            Ok(*response)
        } else {
            Err(ForkchoiceTaskError::InvalidForkchoiceResponse)
        }
    }

    /// Fetches the sync status through the external API.
    pub async fn fetch_sync_status(&mut self) -> Result<SyncStatus, ForkchoiceTaskError> {
        self.sender
            .send(ForkchoiceTaskOut::SyncStatus)
            .map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        let response = self.receiver.recv().await.map_err(|_| ForkchoiceTaskError::ReceiveError)?;
        if let ForkchoiceTaskInput::SyncStatusResponse(response) = response {
            Ok(response)
        } else {
            Err(ForkchoiceTaskError::InvalidSyncStatusResponse)
        }
    }
}

#[async_trait::async_trait]
impl EngineTask for ForkchoiceTask {
    type Error = ForkchoiceTaskError;

    async fn execute(&mut self) -> Result<(), Self::Error> {
        // Check if a forkchoice update is not needed, return early.
        let state = self.fetch_state().await?;
        if !state.forkchoice_update_needed {
            return Err(ForkchoiceTaskError::NoForkchoiceUpdateNeeded);
        }

        // If the engine is syncing, log.
        let sync = self.fetch_sync_status().await?;
        if sync.is_syncing() {
            warn!(target: "engine", "Attempting to update forkchoice state while EL syncing");
        }

        // Check if the head is behind the finalized head.
        let state = self.fetch_state().await?;
        if state.unsafe_head().block_info.number < state.finalized_head().block_info.number {
            return Err(ForkchoiceTaskError::InvalidForkchoiceState(
                state.unsafe_head().block_info.number,
                state.finalized_head().block_info.number,
            ));
        }

        let forkchoice = state.create_forkchoice_state();
        let msg = ForkchoiceTaskOut::ExecuteForkchoiceUpdate(
            EngineForkchoiceVersion::V3,
            forkchoice,
            None,
        );
        self.sender.send(msg).map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        let response = self.receiver.recv().await.map_err(|_| ForkchoiceTaskError::ReceiveError)?;
        let update = match response {
            ForkchoiceTaskInput::ForkchoiceUpdated(update) => update,
            ForkchoiceTaskInput::ForkchoiceUpdateFailed => {
                return Err(ForkchoiceTaskError::ForkchoiceUpdateFailed)
            }
            _ => return Err(ForkchoiceTaskError::InvalidForkchoiceResponse),
        };

        if update.payload_status.is_valid() {
            self.sender
                .send(ForkchoiceTaskOut::ForkchoiceUpdated(update))
                .map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        }

        self.sender
            .send(ForkchoiceTaskOut::UpdateBackupUnsafeHead)
            .map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        self.sender
            .send(ForkchoiceTaskOut::ForkchoiceNotNeeded)
            .map_err(|_| ForkchoiceTaskError::FailedToSend)?;

        Ok(())
    }
}
