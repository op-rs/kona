//! Admin RPC Module

use crate::AdminApiServer;
use alloy_primitives::B256;
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use tokio::sync::oneshot;

/// The query types to the sequencer actor for the admin api.
#[derive(Debug)]
pub enum SequencerAdminQuery {
    /// A query to check if the sequencer is active.
    SequencerActive(oneshot::Sender<bool>),
    /// A query to start the sequencer.
    StartSequencer,
    /// A query to stop the sequencer.
    StopSequencer(oneshot::Sender<B256>),
    /// A query to check if the conductor is enabled.
    ConductorEnabled(oneshot::Sender<bool>),
    /// A query to set the recover mode.
    SetRecoveryMode(bool),
    /// A query to override the leader.
    OverrideLeader,
}

/// The query types to the network actor for the admin api.
#[derive(Debug)]
pub enum NetworkAdminQuery {
    /// An admin rpc request to post an unsafe payload.
    PostUnsafePayload {
        /// The payload to post.
        payload: OpExecutionPayloadEnvelope,
    },
}

type SequencerQuerySender = tokio::sync::mpsc::Sender<SequencerAdminQuery>;
type NetworkAdminQuerySender = tokio::sync::mpsc::Sender<NetworkAdminQuery>;

/// The admin rpc server.
#[derive(Debug)]
pub struct AdminRpc {
    /// The sender to the sequencer actor.
    pub sequencer_sender: Option<SequencerQuerySender>,
    /// The sender to the network actor.
    pub network_sender: NetworkAdminQuerySender,
}

#[async_trait]
impl AdminApiServer for AdminRpc {
    async fn admin_post_unsafe_payload(
        &self,
        payload: OpExecutionPayloadEnvelope,
    ) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_gossip::Metrics::RPC_CALLS, "method" => "admin_postUnsafePayload");
        self.network_sender
            .send(NetworkAdminQuery::PostUnsafePayload { payload })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_sequencer_active(&self) -> RpcResult<bool> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_sender) = self.sequencer_sender else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        let (tx, rx) = oneshot::channel();
        sequencer_sender
            .send(SequencerAdminQuery::SequencerActive(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;
        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_start_sequencer(&self) -> RpcResult<()> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_sender) = self.sequencer_sender else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        sequencer_sender
            .send(SequencerAdminQuery::StartSequencer)
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_stop_sequencer(&self) -> RpcResult<B256> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_sender) = self.sequencer_sender else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        let (tx, rx) = oneshot::channel();

        sequencer_sender
            .send(SequencerAdminQuery::StopSequencer(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;
        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_conductor_enabled(&self) -> RpcResult<bool> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_sender) = self.sequencer_sender else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        let (tx, rx) = oneshot::channel();

        sequencer_sender
            .send(SequencerAdminQuery::ConductorEnabled(tx))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;
        rx.await.map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_set_recover_mode(&self, mode: bool) -> RpcResult<()> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_sender) = self.sequencer_sender else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        sequencer_sender
            .send(SequencerAdminQuery::SetRecoveryMode(mode))
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_override_leader(&self) -> RpcResult<()> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_sender) = self.sequencer_sender else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        sequencer_sender
            .send(SequencerAdminQuery::OverrideLeader)
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }
}
