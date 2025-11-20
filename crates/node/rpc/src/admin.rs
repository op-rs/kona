//! Admin RPC Module

use crate::AdminApiServer;
use alloy_primitives::B256;
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use rollup_boost::{
    ExecutionMode, GetExecutionModeResponse, SetExecutionModeRequest, SetExecutionModeResponse,
};
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

/// The query types to the rollup boost component of the engine actor.
/// Only set when rollup boost is enabled.
#[derive(Debug)]
pub enum RollupBoostAdminQuery {
    /// An admin rpc request to set the execution mode.
    SetExecutionMode {
        /// The execution mode to set.
        execution_mode: ExecutionMode,
    },
    /// An admin rpc request to get the execution mode.
    GetExecutionMode {
        /// The sender to send the execution mode to.
        sender: oneshot::Sender<ExecutionMode>,
    },
}

type SequencerQuerySender = tokio::sync::mpsc::Sender<SequencerAdminQuery>;
type NetworkAdminQuerySender = tokio::sync::mpsc::Sender<NetworkAdminQuery>;
type RollupBoostAdminQuerySender = tokio::sync::mpsc::Sender<RollupBoostAdminQuery>;

/// The admin rpc server.
#[derive(Debug)]
pub struct AdminRpc {
    /// The sender to the sequencer actor.
    pub sequencer_sender: Option<SequencerQuerySender>,
    /// The sender to the network actor.
    pub network_sender: NetworkAdminQuerySender,
    /// The sender to the rollup boost component of the engine actor.
    /// Only set when rollup boost is enabled.
    pub rollup_boost_sender: Option<RollupBoostAdminQuerySender>,
}

impl AdminRpc {
    /// Constructs a new [`AdminRpc`] given the sequencer sender, network sender, and execution
    /// mode.
    ///
    /// # Parameters
    ///
    /// - `sequencer_sender`: The sender to the sequencer actor.
    /// - `network_sender`: The sender to the network actor.
    /// - `rollup_boost_sender`: Sender of admin queries to the rollup boost component of the engine
    ///   actor.
    ///
    /// # Returns
    ///
    /// A new [`AdminRpc`] instance.
    pub const fn new(
        sequencer_sender: Option<SequencerQuerySender>,
        network_sender: NetworkAdminQuerySender,
        rollup_boost_sender: Option<RollupBoostAdminQuerySender>,
    ) -> Self {
        Self { sequencer_sender, network_sender, rollup_boost_sender }
    }
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

    async fn set_execution_mode(
        &self,
        request: SetExecutionModeRequest,
    ) -> RpcResult<SetExecutionModeResponse> {
        let Some(ref rollup_boost_sender) = self.rollup_boost_sender else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        rollup_boost_sender
            .send(RollupBoostAdminQuery::SetExecutionMode {
                execution_mode: request.execution_mode,
            })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;
        Ok(SetExecutionModeResponse { execution_mode: request.execution_mode })
    }

    async fn get_execution_mode(&self) -> RpcResult<GetExecutionModeResponse> {
        let Some(ref rollup_boost_sender) = self.rollup_boost_sender else {
            return Err(ErrorObject::from(ErrorCode::MethodNotFound));
        };

        let (tx, rx) = oneshot::channel();

        rollup_boost_sender
            .send(RollupBoostAdminQuery::GetExecutionMode { sender: tx })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

        rx.await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
            .map(|execution_mode| GetExecutionModeResponse { execution_mode })
    }
}
