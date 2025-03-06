//! The engine actor coordinates tasks that apply to the engine api.
//!
//! These tasks are ephemeral and follow a blocking request-response pattern.
//! Tasks can be spun up in a separate thread and communicate with the engine actor
//! through a channel.
//!
//! This component effectively replaces the `EngineController` from the [op-node][op-node].
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L46

use crate::{EngineClient, ForkchoiceTaskExt, ForkchoiceTaskInput, ForkchoiceTaskOut, SyncStatus};
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
    /// A message from the Forkchoice Task.
    ForkchoiceTask(ForkchoiceTaskOut),
}

/// An error that occurs when running the engine actor.
#[derive(Debug, thiserror::Error)]
pub enum EngineActorError {}

/// The Engine Actor.
#[derive(Debug)]
pub struct EngineActor {
    /// The internal engine client.
    pub client: Arc<EngineClient>,
    /// The sync status.
    pub sync_status: SyncStatus,
    /// A reference to the rollup config.
    pub rollup_config: Arc<RollupConfig>,
    /// A receiver channel to receive messages from the rollup node.
    pub receiver: Receiver<EngineEvent>,
    /// A sender channel to send messages to the rollup node.
    pub sender: Sender<EngineActorMessage>,

    /// The external communication handle to the forkchoice task.
    pub forkchoice_task: Option<ForkchoiceTaskExt>,
}

impl EngineActor {
    /// Creates a new engine actor.
    pub const fn new(
        client: Arc<EngineClient>,
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
            if let Some(ref mut ext) = self.forkchoice_task {
                let receiver = &mut ext.receiver;
                if let Ok(msg) = receiver.try_recv() {
                    trace!(target: "engine", "Received message from forkchoice task: {:?}", msg);
                    self.process_forkchoice_message(msg).await;
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
                self.forkchoice_task = Some(ForkchoiceTaskExt::spawn(self.client.clone()));
            }
        }
        Ok(())
    }

    /// Process a [ForkchoiceTaskOut] message received from the forkchoice task.
    pub async fn process_forkchoice_message(&mut self, msg: ForkchoiceTaskOut) {
        match msg {
            ForkchoiceTaskOut::ExecuteForkchoiceUpdate(_, s, p) => {
                match self.client.fork_choice_updated_v3(s, p).await {
                    Ok(update) => {
                        let sender = self.forkchoice_task.as_ref().map(|ext| ext.sender.clone());
                        let msg = ForkchoiceTaskInput::ForkchoiceUpdated(update);
                        crate::send_until_success_opt!("engine", sender, msg);
                    }
                    Err(_) => {
                        let sender = self.forkchoice_task.as_ref().map(|ext| ext.sender.clone());
                        crate::send_until_success_opt!(
                            "engine",
                            sender,
                            ForkchoiceTaskInput::ForkchoiceUpdateFailed
                        );
                    }
                }
            }
            _ => {
                let msg = EngineActorMessage::ForkchoiceTask(msg);
                crate::send_until_success!("engine", self.sender, msg);
            }
        }
    }
}
