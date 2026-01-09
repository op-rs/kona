use crate::{
    DerivationActorRequest, DerivationClientError, DerivationClientResult, EngineClientError,
    EngineClientResult, actors::engine::EngineActorRequest,
};
use async_trait::async_trait;
use derive_more::Constructor;
use kona_protocol::BlockInfo;
use std::fmt::Debug;
use tokio::sync::mpsc;

/// Trait to be used to interact with the [`crate::EngineActor`], abstracting actual means of
/// communication.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait L1WatcherEngineClient: Debug + Send + Sync {
    /// Sends the [`crate::EngineActor`] the provided finalized L1 block.
    /// Note: this function just guarantees that it is received by the engine but does not have
    /// any insight into whether it was processed or processed successfully.
    async fn send_finalized_l1_block(&self, block: BlockInfo) -> EngineClientResult<()>;
}

/// Queue-based implementation of the [`L1WatcherEngineClient`] trait.
#[derive(Constructor, Debug)]
pub struct QueuedL1WatcherEngineClient {
    /// A channel to use to send the [`crate::EngineActor`] requests.
    pub engine_actor_request_tx: mpsc::Sender<EngineActorRequest>,
}

#[async_trait]
impl L1WatcherEngineClient for QueuedL1WatcherEngineClient {
    async fn send_finalized_l1_block(&self, block: BlockInfo) -> EngineClientResult<()> {
        let _ = self
            .engine_actor_request_tx
            .send(EngineActorRequest::ProcessFinalizedL1BlockRequest(Box::new(block)))
            .await
            .map_err(|_| EngineClientError::RequestError("request channel closed.".to_string()))?;

        Ok(())
    }
}

/// Client to use to interact with the [`crate::DerivationActor`].
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait L1WatcherDerivationClient: Debug + Send + Sync {
    /// Sends the latest L1 Head to the [`crate::DerivationActor`].
    async fn send_new_l1_head(&self, block: BlockInfo) -> DerivationClientResult<()>;
}

/// Client to use to send messages to the Engine Actor's inbound channel.
#[derive(Constructor, Debug)]
pub struct QueuedL1WatcherDerivationClient {
    /// A channel to use to send the [`EngineActorRequest`]s to the EngineActor.
    pub derivation_actor_request_tx: mpsc::Sender<DerivationActorRequest>,
}

#[async_trait]
impl L1WatcherDerivationClient for QueuedL1WatcherDerivationClient {
    async fn send_new_l1_head(&self, block: BlockInfo) -> DerivationClientResult<()> {
        self.derivation_actor_request_tx
            .send(DerivationActorRequest::ProcessL1HeadUpdateRequest(block))
            .await
            .map_err(|_| {
                DerivationClientError::RequestError("request channel closed.".to_string())
            })?;

        Ok(())
    }
}
