//! The engine actor coordinates tasks that apply to the engine api.
//!
//! These tasks are ephemeral and follow a blocking request-response pattern.
//! Tasks can be spun up in a separate thread and communicate with the engine actor
//! through a channel.
//!
//! This component effectively replaces the `EngineController` from the [op-node][op-node].
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L46

use crate::{EngineClient, EngineTask, ForkchoiceMessage, ForkchoiceTask, SyncStatus};
use kona_genesis::RollupConfig;
use op_alloy_provider::ext::engine::OpEngineApi;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use tracing::{trace, warn};

/// A stub event from the consensus node to the engine actor.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Try to update the forkchoice.
    TryUpdateForkchoice,
}

/// A message from the engine actor to the consensus node.
#[derive(Debug, Clone)]
pub enum EngineActorMessage {
    /// A request for a snapshot of the state.
    StateSnapshot,
    /// Request the sync status.
    SyncStatus,
    /// Instruct the state to update the backup unsafe head.
    UpdateBackupUnsafeHead,
    /// Instruct the state that a forkchoice update is not needed.
    ForkchoiceNotNeeded,
}

/// An error that occurs when running the engine actor.
#[derive(Debug, thiserror::Error)]
pub enum EngineActorError {}

/// The Engine Actor.
#[derive(Debug)]
pub struct EngineActor {
    /// The internal engine client.
    pub client: EngineClient,
    /// The sync status.
    pub sync_status: SyncStatus,
    /// A reference to the rollup config.
    pub rollup_config: Arc<RollupConfig>,
    /// A receiver channel to receive messages from the rollup node.
    pub receiver: Receiver<EngineEvent>,
    /// A sender channel to send messages to the rollup node.
    pub sender: Sender<EngineActorMessage>,

    /// A handle to receiver and sender channels for the forkchoice task.
    pub forkchoice_task: Option<(Receiver<ForkchoiceMessage>, Sender<ForkchoiceMessage>)>,
}

impl EngineActor {
    /// Creates a new engine actor.
    pub const fn new(
        client: EngineClient,
        sync_status: SyncStatus,
        rollup_config: Arc<RollupConfig>,
        receiver: Receiver<EngineEvent>,
        sender: Sender<EngineActorMessage>,
    ) -> Self {
        Self { client, sync_status, rollup_config, receiver, sender, forkchoice_task: None }
    }

    /// Runs the engine actor.
    pub async fn start(mut self) -> Result<(), EngineActorError> {
        loop {
            match self.receiver.try_recv() {
                Ok(msg) => self.process(msg).await?,
                Err(_) => warn!(target: "engine", "Failed to receive message from consensus node."),
            }
            if let Some((ref mut receiver, ref _sender)) = self.forkchoice_task {
                if let Ok(msg) = receiver.try_recv() {
                    trace!(target: "engine", "Received message from forkchoice task: {:?}", msg);
                }
            }
        }
    }

    /// Process an event.
    pub async fn process(&mut self, msg: EngineEvent) -> Result<(), EngineActorError> {
        match msg {
            EngineEvent::TryUpdateForkchoice => {
                if self.forkchoice_task.is_some() {
                    warn!(target: "engine", "Forkchoice task already running.");
                    return Ok(());
                }
                let (a_sender, t_receiver) = tokio::sync::broadcast::channel(1);
                let (t_sender, a_receiver) = tokio::sync::broadcast::channel(1);
                self.forkchoice_task = Some((a_receiver, a_sender));
                let mut task = ForkchoiceTask::new(t_receiver, t_sender);
                tokio::spawn(async move { task.execute().await });
            }
        }
        Ok(())
    }

    /// Process a [ForkchoiceMessage] received from the forkchoice task.
    pub async fn process_forkchoice_message(
        &mut self,
        msg: ForkchoiceMessage,
    ) -> Result<(), EngineActorError> {
        match msg {
            ForkchoiceMessage::StateSnapshot => {
                if let Err(e) = self.sender.send(EngineActorMessage::StateSnapshot) {
                    warn!(target: "engine", "Failed to send message to consensus node: {:?}", e);
                }
            }
            ForkchoiceMessage::SyncStatus => {
                if let Err(e) = self.sender.send(EngineActorMessage::SyncStatus) {
                    warn!(target: "engine", "Failed to send message to consensus node: {:?}", e);
                }
            }
            ForkchoiceMessage::StateResponse(_) => {
                warn!(target: "engine", "Unexpected state response from forkchoice task")
            }
            ForkchoiceMessage::SyncStatusResponse(_) => {
                warn!(target: "engine", "Unexpected sync status response from forkchoice task")
            }
            ForkchoiceMessage::ForkchoiceUpdated(_) => {
                warn!(target: "engine", "Unexpected forkchoice task to send forkchoice updated response")
            }
            ForkchoiceMessage::ExecuteForkchoiceUpdate(_, s, p) => {
                // TODO: this will need to be spun into another thread to unblock the actor.
                match self.client.fork_choice_updated_v3(s, p).await {
                    Ok(update) => {
                        // Send the update to the forkchoice task.
                        if let Some((_, ref sender)) = self.forkchoice_task {
                            if let Err(e) =
                                sender.send(ForkchoiceMessage::ForkchoiceUpdated(update))
                            {
                                warn!(target: "engine", "Failed to send message to forkchoice task: {:?}", e);
                            }
                        }
                    }
                    Err(_) => {
                        // TODO: will we need to tell the engine actor that the update failed?
                    }
                }
            }
            ForkchoiceMessage::UpdateBackupUnsafeHead => {
                if let Err(e) = self.sender.send(EngineActorMessage::UpdateBackupUnsafeHead) {
                    warn!(target: "engine", "Failed to send message to consensus node: {:?}", e);
                }
            }
            ForkchoiceMessage::ForkchoiceNotNeeded => {
                if let Err(e) = self.sender.send(EngineActorMessage::ForkchoiceNotNeeded) {
                    warn!(target: "engine", "Failed to send message to consensus node: {:?}", e);
                }
            }
        }
        Ok(())
    }
}
