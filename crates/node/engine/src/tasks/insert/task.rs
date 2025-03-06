//! A task to insert an unsafe payload into the execution engine.

use alloy_eips::BlockNumberOrTag;
use kona_protocol::L2BlockInfo;
use kona_rpc::OpAttributesWithParent;

use crate::{EngineTask, InsertTaskError, InsertTaskInput, InsertTaskOut, SyncStatus};

/// An external handle to communicate with a [InsertTask] spawned
/// in a new thread.
#[derive(Debug)]
pub struct InsertTaskExt {
    /// A receiver channel to receive [InsertTaskOut] events *from* the [InsertTask].
    pub receiver: tokio::sync::broadcast::Receiver<InsertTaskOut>,
    /// A sender channel to send [InsertTaskInput] event *to* the [InsertTask].
    pub sender: tokio::sync::broadcast::Sender<InsertTaskInput>,
    /// A join handle to the spawned thread containing the [InsertTask].
    pub handle: tokio::task::JoinHandle<Result<(), InsertTaskError>>,
}

impl InsertTaskExt {
    /// Spawns the [InsertTask] in a new thread, returning an
    /// external-facing wrapper that can be used to communicate with
    /// the spawned task.
    pub fn spawn(input: (OpAttributesWithParent, L2BlockInfo)) -> Self {
        let (sender, task_receiver) = tokio::sync::broadcast::channel(1);
        let (task_sender, receiver) = tokio::sync::broadcast::channel(1);
        let mut task = InsertTask::new(task_receiver, task_sender);
        let handle = tokio::spawn(async move { task.execute(input).await });
        Self { receiver, sender, handle }
    }
}

/// A strongly typed receiver channel.
type Receiver = tokio::sync::broadcast::Receiver<InsertTaskInput>;

/// A strongly typed sender channel.
type Sender = tokio::sync::broadcast::Sender<InsertTaskOut>;

/// The task to insert an unsafe payload into the execution engine.
#[derive(Debug)]
pub struct InsertTask {
    /// A receiver channel to receive messages from an external actor.
    pub receiver: Receiver,
    /// A sender channel to send messages out to an external actor.
    pub sender: Sender,
}

impl InsertTask {
    /// Creates a new insert task.
    pub const fn new(receiver: Receiver, sender: Sender) -> Self {
        Self { receiver, sender }
    }

    /// Fetches the sync status through the external API.
    pub async fn fetch_sync_status(&mut self) -> Result<SyncStatus, InsertTaskError> {
        crate::send_until_success!("insert", self.sender, InsertTaskOut::SyncStatus);
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::SyncStatusResponse(response) = response {
            Ok(response)
        } else {
            Err(InsertTaskError::InvalidSyncStatusResponse)
        }
    }

    /// Requests an [L2BlockInfo] for the [BlockNumberOrTag::Finalized].
    pub async fn fetch_finalized_info(&mut self) -> Result<L2BlockInfo, InsertTaskError> {
        let msg = InsertTaskOut::L2BlockInfo(BlockNumberOrTag::Finalized);
        crate::send_until_success!("insert", self.sender, msg);
        let response = self.receiver.recv().await.map_err(|_| InsertTaskError::ReceiveFailed)?;
        if let InsertTaskInput::L2BlockInfoResponse(bi) = response {
            Ok(bi)
        } else {
            Err(InsertTaskError::InvalidMessageResponse)
        }
    }
}

#[async_trait::async_trait]
impl EngineTask for InsertTask {
    type Error = InsertTaskError;
    type Input = (OpAttributesWithParent, L2BlockInfo);

    async fn execute(&mut self, _input: Self::Input) -> Result<(), Self::Error> {
        // Request the sync status.
        let sync = self.fetch_sync_status().await?;
        if sync == SyncStatus::ExecutionLayerWillStart {
            match self.fetch_finalized_info().await {
                Ok(_) => {
                    // If there is a finalized block, finish EL sync.
                    let msg = InsertTaskOut::UpdateSyncStatus(SyncStatus::ExecutionLayerFinished);
                    crate::send_until_success!("insert", self.sender, msg);
                }
                Err(_) => {
                    todo!()
                }
            }
        }

        // TODO: Insert Payload in Engine API
        // TODO: Call FCU
        // TODO: Update sync status
        // TODO: Fire off FCU updated event

        Ok(())
    }
}
