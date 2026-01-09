use crate::{EngineActorRequest, EngineClientError, EngineClientResult, ResetRequest};
use async_trait::async_trait;
use derive_more::Constructor;
use kona_protocol::OpAttributesWithParent;
use std::fmt::Debug;
use tokio::sync::mpsc;

/// Client to use to interact with the engine.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DerivationEngineClient: Debug + Send + Sync {
    /// Resets the engine's forkchoice.
    async fn reset_engine_forkchoice(&self) -> EngineClientResult<()>;

    /// Sends the derived attributes to the Engine.
    /// Note: This does not wait for the engine to process them.
    async fn send_derived_attributes(
        &self,
        attributes: OpAttributesWithParent,
    ) -> EngineClientResult<()>;
}

/// Client to use to send messages to the Engine Actor's inbound channel.
#[derive(Constructor, Debug)]
pub struct QueuedDerivationEngineClient {
    /// A channel to use to send the [`EngineActorRequest`]s to the EngineActor.
    pub engine_actor_request_tx: mpsc::Sender<EngineActorRequest>,
}

#[async_trait]
impl DerivationEngineClient for QueuedDerivationEngineClient {
    async fn reset_engine_forkchoice(&self) -> EngineClientResult<()> {
        let (result_tx, mut result_rx) = mpsc::channel(1);

        info!(target: "derivation", "Sending reset request to engine.");
        self.engine_actor_request_tx
            .send(EngineActorRequest::ResetRequest(Box::new(ResetRequest { result_tx })))
            .await
            .map_err(|_| EngineClientError::RequestError("request channel closed.".to_string()))?;

        result_rx
            .recv()
            .await
            .inspect(|_| info!(target: "derivation", "Engine reset successfully."))
            .ok_or_else(|| {
                error!(target: "derivation_engine_client", "Failed to receive built payload");
                EngineClientError::ResponseError("response channel closed.".to_string())
            })?
    }

    async fn send_derived_attributes(
        &self,
        attributes: OpAttributesWithParent,
    ) -> EngineClientResult<()> {
        trace!(target: "derivation", ?attributes, "Sending derived attributes to engine.");
        self.engine_actor_request_tx
            .send(EngineActorRequest::ProcessDerivedL2AttributesRequest(Box::new(attributes)))
            .await
            .map_err(|_| EngineClientError::RequestError("request channel closed.".to_string()))?;

        Ok(())
    }
}
