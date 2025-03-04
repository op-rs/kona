//! A task for the `engine_forkchoiceUpdated` query.

use alloy_rpc_types_engine::{ForkchoiceState, ForkchoiceUpdated};
use async_trait::async_trait;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use tokio::sync::broadcast::{Receiver, Sender};
use tracing::warn;

use crate::{EngineForkchoiceVersion, EngineState, EngineTask, SyncStatus};

/// An error that occurs when running the forkchoice task.
#[derive(Debug, thiserror::Error)]
pub enum ForkchoiceTaskError {
    /// The forkchoice update is not needed.
    #[error("No forkchoice update needed")]
    NoForkchoiceUpdateNeeded,
    /// The forkchoice state is invalid.
    #[error("Invalid forkchoice state: {0} < {1}")]
    InvalidForkchoiceState(u64, u64),
    /// The forkchoice response is invalid.
    #[error("Invalid forkchoice response")]
    InvalidForkchoiceResponse,
    /// The sync status response is invalid.
    #[error("Invalid sync status response")]
    InvalidSyncStatusResponse,
    /// Failed to send a message to the engine actor.
    #[error("Failed to send message to engine actor")]
    FailedToSend,
    /// A receive error occurred.
    #[error("Receive error")]
    ReceiveError,
}

/// A forkchoice update message.
#[derive(Debug, Clone)]
pub enum ForkchoiceMessage {
    /// A request for a snapshot of the state.
    StateSnapshot,
    /// Request the sync status.
    SyncStatus,
    /// A response from the state request.
    ///
    /// This contains a snapshot of the state.
    StateResponse(Box<EngineState>),
    /// A response from the sync status request.
    SyncStatusResponse(SyncStatus),
    /// The forkchoice was successfully updated, with a valid payload response.
    ForkchoiceUpdated(ForkchoiceUpdated),
    /// A message to update the forkchoice.
    ExecuteForkchoiceUpdate(EngineForkchoiceVersion, ForkchoiceState, Option<OpPayloadAttributes>),
    /// Instruct the state to update the backup unsafe head.
    UpdateBackupUnsafeHead,
    /// Instruct the state that a forkchoice update is not needed.
    ForkchoiceNotNeeded,
}

/// The task to update the forkchoice.
#[derive(Debug)]
pub struct ForkchoiceTask {
    /// A receiver channel to receive messages from the engine actor.
    pub receiver: Receiver<ForkchoiceMessage>,
    /// A sender channel to send messages to the engine actor.
    pub sender: Sender<ForkchoiceMessage>,
}

impl ForkchoiceTask {
    /// Creates a new forkchoice task.
    pub const fn new(
        receiver: Receiver<ForkchoiceMessage>,
        sender: Sender<ForkchoiceMessage>,
    ) -> Self {
        Self { receiver, sender }
    }

    /// Fetches a state snapshot through the external API.
    pub async fn fetch_state(&mut self) -> Result<EngineState, ForkchoiceTaskError> {
        self.sender
            .send(ForkchoiceMessage::StateSnapshot)
            .map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        let response = self.receiver.recv().await.map_err(|_| ForkchoiceTaskError::ReceiveError)?;
        if let ForkchoiceMessage::StateResponse(response) = response {
            Ok(*response)
        } else {
            Err(ForkchoiceTaskError::InvalidForkchoiceResponse)
        }
    }

    /// Fetches the sync status through the external API.
    pub async fn fetch_sync_status(&mut self) -> Result<SyncStatus, ForkchoiceTaskError> {
        self.sender
            .send(ForkchoiceMessage::SyncStatus)
            .map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        let response = self.receiver.recv().await.map_err(|_| ForkchoiceTaskError::ReceiveError)?;
        if let ForkchoiceMessage::SyncStatusResponse(response) = response {
            Ok(response)
        } else {
            Err(ForkchoiceTaskError::InvalidSyncStatusResponse)
        }
    }
}

#[async_trait]
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
        let msg = ForkchoiceMessage::ExecuteForkchoiceUpdate(
            EngineForkchoiceVersion::V3,
            forkchoice,
            None,
        );
        self.sender.send(msg).map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        let response = self.receiver.recv().await.map_err(|_| ForkchoiceTaskError::ReceiveError)?;
        let update = match response {
            ForkchoiceMessage::ForkchoiceUpdated(update) => update,
            _ => return Err(ForkchoiceTaskError::InvalidForkchoiceResponse),
        };

        if update.payload_status.is_valid() {
            self.sender
                .send(ForkchoiceMessage::ForkchoiceUpdated(update))
                .map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        }

        self.sender
            .send(ForkchoiceMessage::UpdateBackupUnsafeHead)
            .map_err(|_| ForkchoiceTaskError::FailedToSend)?;
        self.sender
            .send(ForkchoiceMessage::ForkchoiceNotNeeded)
            .map_err(|_| ForkchoiceTaskError::FailedToSend)?;

        Ok(())
    }
}
