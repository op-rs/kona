use crate::{EngineClientError, EngineClientResult, actors::engine::EngineActorRequest};
use async_trait::async_trait;
use derive_more::Constructor;
use kona_protocol::BlockInfo;
use std::fmt::Debug;
use tokio::sync::mpsc;

/// Trait to be used to interact with the EngineActor, abstracting actual means of communication.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait L1WatcherEngineClient: Debug + Send + Sync {
    /// Sends the engine the provided finalized L1 block.
    /// Note: this function just guarantees that the finalized L1 block is received by the engine
    /// but does not have any insight into whether the engine successfully finalized the block.
    async fn send_finalized_l1_block(&self, block: BlockInfo) -> EngineClientResult<()>;
}

/// Queue-based implementation of the [`L1WatcherEngineClient`] trait.
#[derive(Constructor, Debug)]
pub struct QueuedL1WatcherEngineClient {
    /// A channel to use to send the EngineActor requests.
    pub engine_actor_request_tx: mpsc::Sender<EngineActorRequest>,
}

#[async_trait]
impl L1WatcherEngineClient for QueuedL1WatcherEngineClient {
    async fn send_finalized_l1_block(&self, block: BlockInfo) -> EngineClientResult<()> {
        Ok(self
            .engine_actor_request_tx
            .send(EngineActorRequest::ProcessFinalizedL1BlockRequest(Box::new(block)))
            .await
            .map_err(|_| EngineClientError::RequestError("request channel closed.".to_string()))?)
    }
}
