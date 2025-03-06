//! The engine actor coordinates tasks that apply to the engine api.
//!
//! These tasks are ephemeral and follow a blocking request-response pattern.
//! Tasks can be spun up in a separate thread and communicate with the engine actor
//! through a channel.
//!
//! This component effectively replaces the `EngineController` from the [op-node][op-node].
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L46

use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};

use crate::{
    EngineClient, EngineForkchoiceVersion, ForkchoiceTaskExt, ForkchoiceTaskOut, InsertTaskExt,
    InsertTaskOut, SyncConfig, SyncStatus,
};

/// A stub event from the consensus node to the engine actor.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Try to update the forkchoice.
    TryUpdateForkchoice,
    /// A message to insert an unsafe payload.
    InsertUnsafePayload(OpNetworkPayloadEnvelope, L2BlockInfo),
}

/// A message from the engine actor to the consensus node.
#[derive(Debug, Clone)]
pub enum EngineActorMessage {
    /// A message from the Forkchoice Task.
    ForkchoiceTask(ForkchoiceTaskOut),
    /// A message from the Insert Unsafe Payload Task.
    InsertTask(InsertTaskOut),
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
    /// The sync configuration.
    pub sync_config: Arc<SyncConfig>,
    /// A reference to the rollup config.
    pub rollup_config: Arc<RollupConfig>,
    /// A receiver channel to receive messages from the rollup node.
    pub receiver: Receiver<EngineEvent>,
    /// A sender channel to send messages to the rollup node.
    pub sender: Sender<EngineActorMessage>,

    /// The external communication handle to the forkchoice task.
    pub forkchoice_task: Option<ForkchoiceTaskExt>,
    /// The external communication handle to the insert task.
    pub insert_task: Option<InsertTaskExt>,
}

impl EngineActor {
    /// Creates a new engine actor.
    pub const fn new(
        client: Arc<EngineClient>,
        sync_status: SyncStatus,
        sync_config: Arc<SyncConfig>,
        rollup_config: Arc<RollupConfig>,
        receiver: Receiver<EngineEvent>,
        sender: Sender<EngineActorMessage>,
    ) -> Self {
        Self {
            client,
            sync_status,
            sync_config,
            rollup_config,
            receiver,
            sender,
            forkchoice_task: None,
            insert_task: None,
        }
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
                    let msg = EngineActorMessage::ForkchoiceTask(msg);
                    crate::send_until_success!("engine", self.sender, msg);
                }
            }
            if let Some(ref mut ext) = self.insert_task {
                let receiver = &mut ext.receiver;
                if let Ok(msg) = receiver.try_recv() {
                    trace!(target: "engine", "Received message from insert task: {:?}", msg);
                    let msg = EngineActorMessage::InsertTask(msg);
                    crate::send_until_success!("engine", self.sender, msg);
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
            EngineEvent::InsertUnsafePayload(envelope, info) => {
                if self.insert_task.is_some() {
                    warn!(target: "engine", "Insert task already running.");
                    return Ok(());
                }
                let version = EngineForkchoiceVersion::from_cfg(
                    &self.rollup_config,
                    info.block_info.timestamp,
                );
                let input = (
                    self.client.clone(),
                    self.sync_config.clone(),
                    self.rollup_config.genesis.l2.hash,
                    version,
                    envelope,
                    info,
                );
                self.insert_task = Some(InsertTaskExt::spawn(input));
            }
        }
        Ok(())
    }
}
