//! A task for the `engine_forkchoiceUpdated` query.

use op_alloy_provider::ext::engine::OpEngineApi;
use std::sync::Arc;

use crate::{
    EngineClient, EngineState, EngineTask, ForkchoiceTaskError, ForkchoiceTaskInput,
    ForkchoiceTaskOut, SyncStatus,
};

/// The input type for the [ForkchoiceTask].
type Input = Arc<EngineClient>;

/// An external handle to communicate with a [ForkchoiceTask] spawned
/// in a new thread.
#[derive(Debug)]
pub struct ForkchoiceTaskExt {
    /// A receiver channel to receive [ForkchoiceTaskOut] events *from* the [ForkchoiceTask].
    pub receiver: tokio::sync::broadcast::Receiver<ForkchoiceTaskOut>,
    /// A sender channel to send [ForkchoiceTaskInput] event *to* the [ForkchoiceTask].
    pub sender: tokio::sync::broadcast::Sender<ForkchoiceTaskInput>,
    /// A join handle to the spawned thread containing the [ForkchoiceTask].
    pub handle: tokio::task::JoinHandle<Result<(), ForkchoiceTaskError>>,
}

impl ForkchoiceTaskExt {
    /// Spawns the [ForkchoiceTask] in a new thread, returning an
    /// external-facing wrapper that can be used to communicate with
    /// the spawned task.
    pub fn spawn(input: Input) -> Self {
        let (sender, task_receiver) = tokio::sync::broadcast::channel(1);
        let (task_sender, receiver) = tokio::sync::broadcast::channel(1);
        let mut task = ForkchoiceTask::new(task_receiver, task_sender);
        let handle = tokio::spawn(async move { task.execute(input).await });
        Self { receiver, sender, handle }
    }
}

/// The receiver type for the forkchoice task.
type TaskReceiver = tokio::sync::broadcast::Receiver<ForkchoiceTaskInput>;

/// The sender type for the forkchoice task.
type TaskSender = tokio::sync::broadcast::Sender<ForkchoiceTaskOut>;

/// The task to update the forkchoice.
#[derive(Debug)]
pub struct ForkchoiceTask {
    /// A receiver channel to receive messages from an external actor.
    pub receiver: TaskReceiver,
    /// A sender channel to send messages out to an external actor.
    pub sender: TaskSender,
}

impl ForkchoiceTask {
    /// Creates a new forkchoice task.
    pub const fn new(receiver: TaskReceiver, sender: TaskSender) -> Self {
        Self { receiver, sender }
    }

    /// Fetches a state snapshot through the external API.
    pub async fn fetch_state(&mut self) -> Result<EngineState, ForkchoiceTaskError> {
        crate::send_until_success!("fcu", self.sender, ForkchoiceTaskOut::StateSnapshot);
        let response = self.receiver.recv().await.map_err(|_| ForkchoiceTaskError::ReceiveError)?;
        if let ForkchoiceTaskInput::StateResponse(response) = response {
            Ok(*response)
        } else {
            Err(ForkchoiceTaskError::InvalidForkchoiceResponse)
        }
    }

    /// Fetches the sync status through the external API.
    pub async fn fetch_sync_status(&mut self) -> Result<SyncStatus, ForkchoiceTaskError> {
        crate::send_until_success!("fcu", self.sender, ForkchoiceTaskOut::SyncStatus);
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
    type Input = Input;

    async fn execute(&mut self, input: Self::Input) -> Result<(), Self::Error> {
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

        // Send the forkchoice update through the input.
        let forkchoice = state.create_forkchoice_state();
        let update = input
            .fork_choice_updated_v3(forkchoice, None)
            .await
            .map_err(|_| ForkchoiceTaskError::ForkchoiceUpdateFailed)?;

        if update.payload_status.is_valid() {
            let msg = ForkchoiceTaskOut::ForkchoiceUpdated(update);
            crate::send_until_success!("fcu", self.sender, msg);
        }

        // TODO: The state actor will need to handle this update.
        // TODO: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L360-L363
        crate::send_until_success!("fcu", self.sender, ForkchoiceTaskOut::UpdateBackupUnsafeHead);
        crate::send_until_success!("fcu", self.sender, ForkchoiceTaskOut::ForkchoiceNotNeeded);

        Ok(())
    }
}
