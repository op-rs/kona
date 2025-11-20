use crate::actors::engine::{BuildRequest, SealRequest, actor::ResetRequest};
use alloy_rpc_types_engine::PayloadId;
use async_trait::async_trait;
use derive_more::Constructor;
use kona_engine::{BuildTaskError, SealTaskError};
use kona_protocol::OpAttributesWithParent;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::{fmt::Debug, time::Instant};
use thiserror::Error;
use tokio::sync::mpsc;

/// Trait to be referenced by those interacting with EngineActor for block building
/// operations. The EngineActor requires the use of channels for communication, but
/// this interface allows that to be abstracted from callers and allows easy testing.
#[async_trait]
pub trait BlockEngineClient: Debug + Send + Sync {
    /// Resets the engine's forkchoice, awaiting confirmation that it succeeded or returning the
    /// error in performing the reset.
    async fn reset_engine_forkchoice(&self) -> BlockEngineResult<()>;

    /// Starts building a block with the provided attributes.
    ///
    /// Returns a `PayloadId` that can be used to seal the block later.
    async fn start_build_block(
        &self,
        attributes: OpAttributesWithParent,
    ) -> BlockEngineResult<PayloadId>;

    /// Seals and canonicalizes a previously started block.
    ///
    /// Takes a `PayloadId` from a previous `start_build_block` call and returns
    /// the finalized execution payload envelope.
    async fn seal_and_canonicalize_block(
        &self,
        payload_id: PayloadId,
        attributes: OpAttributesWithParent,
    ) -> BlockEngineResult<OpExecutionPayloadEnvelope>;
}

/// Queue-based implementation of the [`BlockEngineClient`] trait. This handles all channel-based
/// operations, providing a nice facade for callers.
#[derive(Constructor, Debug)]
pub struct QueuedBlockEngineClient {
    /// A channel to use to send build requests to the engine.
    /// Upon successful processing of the provided attributes, a `PayloadId` will be sent via the
    /// provided sender.
    pub build_request_tx: mpsc::Sender<BuildRequest>,
    /// A channel to send seal requests to the engine.
    /// If provided, the success/fail result of the sealing operation will be sent via the provided
    /// sender.
    pub seal_request_tx: mpsc::Sender<SealRequest>,
    /// A channel to send reset requests to the engine.
    /// If provided, the success/fail result of the reset operation will be sent via the provided
    /// sender.
    pub reset_request_tx: mpsc::Sender<ResetRequest>,
}

#[async_trait]
impl BlockEngineClient for QueuedBlockEngineClient {
    async fn reset_engine_forkchoice(&self) -> BlockEngineResult<()> {
        let (result_tx, mut result_rx) = mpsc::channel(1);

        if self.reset_request_tx.send(ResetRequest { result_tx: Some(result_tx) }).await.is_err() {
            return Err(BlockEngineError::RequestError)
        }

        match result_rx.recv().await {
            Some(Ok(())) => Ok(()),
            Some(Err(x)) => Err(x),
            None => {
                error!(target: "block_engine", "Failed to receive built payload");
                Err(BlockEngineError::ResponseError)
            }
        }
    }

    async fn start_build_block(
        &self,
        attributes: OpAttributesWithParent,
    ) -> BlockEngineResult<PayloadId> {
        let (payload_id_tx, mut payload_id_rx) = mpsc::channel(1);

        if self
            .build_request_tx
            .send(BuildRequest { attributes, result_tx: payload_id_tx })
            .await
            .is_err()
        {
            return Err(BlockEngineError::RequestError);
        }

        payload_id_rx.recv().await.ok_or_else(|| {
            error!(target: "block_engine", "Failed to receive payload for initiated block build");
            BlockEngineError::ResponseError
        })
    }

    async fn seal_and_canonicalize_block(
        &self,
        payload_id: PayloadId,
        attributes: OpAttributesWithParent,
    ) -> BlockEngineResult<OpExecutionPayloadEnvelope> {
        let (result_tx, mut result_rx) = mpsc::channel(1);

        if self
            .seal_request_tx
            .send(SealRequest { payload_id, attributes, result_tx })
            .await
            .is_err()
        {
            return Err(BlockEngineError::RequestError)
        }

        match result_rx.recv().await {
            Some(Ok(payload)) => Ok(payload),
            Some(Err(err)) => Err(BlockEngineError::SealError(err)),
            None => {
                error!(target: "block_engine", "Failed to receive built payload");
                Err(BlockEngineError::ResponseError)
            }
        }
    }
}

/// The result of a [`BlockEngineClient`] call.
pub type BlockEngineResult<T> = Result<T, BlockEngineError>;

/// Error making requests to the BlockEngine.
#[derive(Debug, Error)]
pub enum BlockEngineError {
    /// Error making a request to the engine. The request never made it there.
    #[error("Error making a request to the engine.")]
    RequestError,

    /// Error receiving response from the engine.
    /// This means the request may or may not have succeeded.
    #[error("Error receiving response from the engine.")]
    ResponseError,

    /// An error occurred starting to build a block.
    #[error(transparent)]
    StartBuildError(#[from] BuildTaskError),

    /// An error occurred sealing a block.
    #[error(transparent)]
    SealError(#[from] SealTaskError),

    /// An error occurred performing the reset.
    #[error("An error occurred performing the reset.")]
    ResetForkchoiceError,
}
