//! The RPC server for the sequencer actor.
//! Mostly handles queries from the admin rpc.

use alloy_primitives::B256;
use async_trait::async_trait;
use derive_more::Constructor;
use thiserror::Error;
use tokio::select;
use kona_derive::AttributesBuilder;
use kona_protocol::L2BlockInfo;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;
use crate::actors::sequencer::{
    actor::SequencerActorState, conductor::Conductor, origin_selector::OriginSelector,
};
use crate::actors::sequencer::api::{SequencerAdminAPIError, SequencerAdminAPI, SequencerAdminAPIServer, SequencerAdminAPIServerError};


#[derive(Constructor)]
pub struct QueuedSequencerAdminAPIServer<API: SequencerAdminAPI> {
    /// Queue used to receive admin queries.
    request_rx: mpsc::Receiver<SequencerAdminQuery>,
    /// API used to fulfill admin queries.
    api: API,
}

#[async_trait]
impl<API: SequencerAdminAPI> SequencerAdminAPIServer for QueuedSequencerAdminAPIServer<API> {
    async fn serve(&mut self, cancellation_token: CancellationToken) -> Result<(), SequencerAdminAPIServerError> {
        loop {
            select! {
                _ = cancellation_token.cancelled() => {
                    info!(target: "sequencer_queued_admin_api_server", "Received shutdown signal");
                    return Ok(());
                },
                res = self.request_rx.recv().await, !if self.request_rx.is_closed() => {
                    match res {
                        None =>  {
                            return Err(SequencerAdminAPIServerError::Fatal)
                        },
                        Some(SequencerAdminQuery::SequencerActive(tx)) => {
                            tx.send(self.api.is_sequencer_active().await).map_err(|_| warn!(target: "sequencer_queued_admin_api_server", "Response channel closed"));
                        },
                        Some(SequencerAdminQuery::StartSequencer(tx)) => {
                            tx.send(self.api.start_sequencer().await).map_err(|_| warn!(target: "sequencer_queued_admin_api_server", "Response channel closed"));
                        },
                        Some(SequencerAdminQuery::StopSequencer(tx)) => {
                            tx.send(self.api.stop_sequencer().await).map_err(|_| warn!(target: "sequencer_queued_admin_api_server", "Response channel closed"));
                        },
                        Some(SequencerAdminQuery::ConductorEnabled(tx)) => {
                            tx.send(self.api.is_conductor_enabled().await).map_err(|_| warn!(target: "sequencer_queued_admin_api_server", "Response channel closed"));
                        },
                        Some(SequencerAdminQuery::SetRecoveryMode(is_active, tx)) => {
                            tx.send(self.api.set_recovery_mode(is_active).await).map_err(|_| warn!(target: "sequencer_queued_admin_api_server", "Response channel closed"));
                        },
                        Some(SequencerAdminQuery::OverrideLeader(tx)) => {
                            tx.send(self.api.override_leader().await).map_err(|_| warn!(target: "sequencer_queued_admin_api_server", "Response channel closed"));
                        },
                    }
                }
            }
        }
    }
}


#[derive(Constructor)]
pub struct QueuedSequencerAdminAPI {
    /// Queue used to relay admin queries
    request_tx: mpsc::Sender<SequencerAdminQuery>
}

#[derive(Debug, Error)]
pub enum QueuedRequestError {
    /// Error sending request.
    #[error("Error sending request.")]
    RequestError,

    /// Error receiving response.
    #[error("Error receiving response.")]
    ResponseError,
}


/// The query types to the sequencer actor for the admin api.
#[derive(Debug)]
pub enum SequencerAdminQuery {
    /// A query to check if the sequencer is active.
    SequencerActive(oneshot::Sender<Result<bool, SequencerAdminAPIError>>),
    /// A query to start the sequencer.
    StartSequencer(oneshot::Sender<Result<(), SequencerAdminAPIError>>),
    /// A query to stop the sequencer.
    StopSequencer(oneshot::Sender<Result<B256, SequencerAdminAPIError>>),
    /// A query to check if the conductor is enabled.
    ConductorEnabled(oneshot::Sender<Result<bool, SequencerAdminAPIError>>),
    /// A query to set the recover mode.
    SetRecoveryMode(bool, oneshot::Sender<Result<(), SequencerAdminAPIError>>),
    /// A query to override the leader.
    OverrideLeader(oneshot::Sender<Result<(), SequencerAdminAPIError>>),
}

#[async_trait]
impl SequencerAdminAPI for QueuedSequencerAdminAPI {
    async fn is_sequencer_active(&self) -> Result<bool, SequencerAdminAPIError> {
        let (tx, rx) = oneshot::channel();

        self.request_tx
            .send(SequencerAdminQuery::SequencerActive(tx))
            .await
            .map_err(|_| QueuedRequestError::RequestError)?;
        rx.await.map_err(|_| QueuedRequestError::ResponseError)?
    }

    async fn is_conductor_enabled(&self) -> Result<bool, SequencerAdminAPIError> {
        let (tx, rx) = oneshot::channel();

        self.request_tx
            .send(SequencerAdminQuery::ConductorEnabled(tx))
            .await
            .map_err(|_| QueuedRequestError::RequestError)?;
        rx.await.map_err(|_| QueuedRequestError::ResponseError)?
    }

    async fn start_sequencer(&self) -> Result<(), SequencerAdminAPIError> {
        let (tx, rx) = oneshot::channel();

        self.request_tx
            .send(SequencerAdminQuery::StartSequencer(tx))
            .await
            .map_err(|_| QueuedRequestError::RequestError)?;
        rx.await.map_err(|_| QueuedRequestError::ResponseError)?
    }

    async fn stop_sequencer(&self) -> Result<B256, SequencerAdminAPIError> {
        let (tx, rx) = oneshot::channel();

        self.request_tx
            .send(SequencerAdminQuery::StopSequencer(tx))
            .await
            .map_err(|_| QueuedRequestError::RequestError)?;
        rx.await.map_err(|_| QueuedRequestError::ResponseError)?
    }

    async fn set_recovery_mode(&self, mode: bool) -> Result<(), SequencerAdminAPIError> {
        let (tx, rx) = oneshot::channel();

        self.request_tx
            .send(SequencerAdminQuery::SetRecoveryMode(mode, tx))
            .await
            .map_err(|_| QueuedRequestError::RequestError)?;
        rx.await.map_err(|_| QueuedRequestError::ResponseError)?
    }

    async fn override_leader(&self) -> Result<(), SequencerAdminAPIError> {
        let (tx, rx) = oneshot::channel();

        self.request_tx
            .send(SequencerAdminQuery::OverrideLeader(tx))
            .await
            .map_err(|_| QueuedRequestError::RequestError)?;
        rx.await.map_err(|_| QueuedRequestError::ResponseError)?
    }
}

