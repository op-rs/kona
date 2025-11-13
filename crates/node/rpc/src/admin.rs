//! Admin RPC Module

use crate::AdminApiServer;
use alloy_primitives::B256;
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use thiserror::Error;

/// The query types to the network actor for the admin api.
#[derive(Debug)]
pub enum NetworkAdminQuery {
    /// An admin rpc request to post an unsafe payload.
    PostUnsafePayload {
        /// The payload to post.
        payload: OpExecutionPayloadEnvelope,
    },
}

type NetworkAdminQuerySender = tokio::sync::mpsc::Sender<NetworkAdminQuery>;

/// The admin rpc server.
pub struct AdminRpc<S: SequencerAdminAPIClient> {
    /// The sequencer admin API client.
    pub sequencer_admin_client: Option<S>,
    /// The sender to the network actor.
    pub network_sender: NetworkAdminQuerySender,
}

#[async_trait]
impl<S: SequencerAdminAPIClient + Send + Sync + 'static> AdminApiServer for AdminRpc<S> {
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
        let Some(ref sequencer_client) = self.sequencer_admin_client else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        sequencer_client
            .is_sequencer_active()
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_start_sequencer(&self) -> RpcResult<()> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_client) = self.sequencer_admin_client else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        sequencer_client
            .start_sequencer()
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_stop_sequencer(&self) -> RpcResult<B256> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_client) = self.sequencer_admin_client else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        sequencer_client
            .stop_sequencer()
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_conductor_enabled(&self) -> RpcResult<bool> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_client) = self.sequencer_admin_client else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        sequencer_client
            .is_conductor_enabled()
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_set_recover_mode(&self, mode: bool) -> RpcResult<()> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_client) = self.sequencer_admin_client else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        sequencer_client
            .set_recovery_mode(mode)
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }

    async fn admin_override_leader(&self) -> RpcResult<()> {
        // If the sequencer is not enabled (mode runs in validator mode), return an error.
        let Some(ref sequencer_client) = self.sequencer_admin_client else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        sequencer_client
            .override_leader()
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }
}


/// The admin API client for the sequencer actor.
#[async_trait]
pub trait SequencerAdminAPIClient: Clone + Send {
    /// Check if the sequencer is active.
    async fn is_sequencer_active(&self) -> Result<bool, SequencerAdminAPIError>;

    /// Check if the conductor is enabled.
    async fn is_conductor_enabled(&self) -> Result<bool, SequencerAdminAPIError>;

    /// Start the sequencer.
    async fn start_sequencer(&self) -> Result<(), SequencerAdminAPIError>;

    /// Stop the sequencer.
    async fn stop_sequencer(&self) -> Result<B256, SequencerAdminAPIError>;

    /// Set recovery mode.
    async fn set_recovery_mode(&self, mode: bool) -> Result<(), SequencerAdminAPIError>;

    /// Override the leader.
    async fn override_leader(&self) -> Result<(), SequencerAdminAPIError>;
}

/// Errors that can occur when using the sequencer admin API.
#[derive(Debug, Error)]
pub enum SequencerAdminAPIError {
    /// Error sending request.
    #[error("Error sending request.")]
    RequestError,

    /// Error receiving response.
    #[error("Error receiving response.")]
    ResponseError,

    /// Error overriding leader.
    #[error("Error overriding leader.")]
    LeaderOverrideError,
}