use crate::actors::engine::{BuildRequest, SealRequest};
use alloy_rpc_types_engine::PayloadId;
use async_trait::async_trait;
use kona_engine::{BuildError, SealError};
use kona_protocol::OpAttributesWithParent;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::{fmt::Debug, time::Instant};
use derive_more::Constructor;
use tokio::sync::mpsc;

/// Trait to be referenced by those interacting with EngineActor for block building
/// operations. The EngineActor requires the use of channels for communication, but
/// this interface allows that to be abstracted from callers and allows easy testing.
#[async_trait]
pub trait BlockEngine: Debug + Send + Sync {
    async fn start_build_block(
        &self,
        attributes: OpAttributesWithParent,
    ) -> Result<PayloadId, BuildError>;
    async fn seal_and_canonicalize_block(
        &self,
        payload_id: PayloadId,
        attributes: OpAttributesWithParent,
    ) -> Result<OpExecutionPayloadEnvelope, SealError>;
}

/// Queue-based implementation of the [`BlockEngine`] trait. This handles all channel-based operations,
/// providing a nice facade for callers.
#[derive(Constructor,Debug)]
pub struct QueuedBlockEngine {
    /// A channel to use to send build requests to the engine.
    /// Upon successful processing of the provided attributes, a `PayloadId` will be sent via the
    /// provided sender.
    /// ## Note
    /// This is `Some` when the node is in sequencer mode, and `None` when the node is in validator
    /// mode.
    pub build_request_tx: mpsc::Sender<BuildRequest>,
    /// A channel to send seal requests to the engine.
    /// If provided, the success/fail result of the sealing operation will be sent via the provided
    /// sender.
    /// ## Note
    /// This is `Some` when the node is in sequencer mode, and `None` when the node is in validator
    /// mode.
    pub seal_request_tx: mpsc::Sender<SealRequest>,
}

#[async_trait]
impl BlockEngine for QueuedBlockEngine {
    async fn start_build_block(
        &self,
        attributes: OpAttributesWithParent,
    ) -> Result<PayloadId, BuildError> {
        let (payload_id_tx, mut payload_id_rx) = mpsc::channel(1);

        let _build_request_start = Instant::now();
        if let Err(_) = self.build_request_tx.send((attributes, payload_id_tx)).await {
            return Err(BuildError::CommunicationError);
        }

        match payload_id_rx.recv().await {
            Some(payload_id) => Ok(payload_id),
            // TODO: handle errors
            None => {
                error!(target: "block_engine", "Failed to receive payload for initiated block build");
                Err(BuildError::CommunicationError)
            }
        }
    }

    async fn seal_and_canonicalize_block(
        &self,
        payload_id: PayloadId,
        attributes: OpAttributesWithParent,
    ) -> Result<OpExecutionPayloadEnvelope, SealError> {
        let (payload_tx, mut payload_rx) = mpsc::channel(1);

        let _build_request_start = Instant::now();
        if let Err(_) = self.seal_request_tx.send((payload_id, attributes, payload_tx)).await {
            return Err(SealError::CommunicationError);
        }

        match payload_rx.recv().await {
            Some(Ok(x)) => Ok(x),
            Some(Err(x)) => Err(x),
            None => {
                error!(target: "block_engine", "Failed to receive built payload");
                Err(SealError::CommunicationError)
            }
        }
    }
}
