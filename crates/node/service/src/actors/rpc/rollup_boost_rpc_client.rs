use crate::{EngineActorRequest, EngineRpcRequest};
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use kona_rpc::{RollupBoostAdminClient, RollupBoostHealthzApiServer, RollupBoostHealthzResponse};
use rollup_boost::{GetExecutionModeResponse, SetExecutionModeRequest, SetExecutionModeResponse};
use std::fmt::Debug;
use tokio::sync::{mpsc, oneshot};

/// [`RollupBoostHealthzApiServer`] implementation to send the request to EngineActor's request
/// channel.
#[derive(Debug)]
pub struct RollupBoostHealthRpcClient {
    /// A channel to use to send the EngineActor requests.
    pub engine_actor_request_tx: mpsc::Sender<EngineActorRequest>,
}

#[async_trait]
impl RollupBoostHealthzApiServer for RollupBoostHealthRpcClient {
    async fn rollup_boost_healthz(&self) -> RpcResult<RollupBoostHealthzResponse> {
        let (health_tx, health_rx) = oneshot::channel();

        if self
            .engine_actor_request_tx
            .send(EngineActorRequest::RpcRequest(EngineRpcRequest::RollupBoostHealthRequest(
                kona_rpc::RollupBoostHealthQuery { sender: health_tx },
            )))
            .await
            .is_err()
        {
            return Err(ErrorObject::from(ErrorCode::InternalError));
        }

        health_rx.await.map_err(|_| {
            error!(target: "block_engine", "Failed to receive rollup boost health from engine rpc");
            ErrorObject::from(ErrorCode::InternalError)
        }).map(|resp| RollupBoostHealthzResponse{rollup_boost_health: resp})
    }
}

/// [`RollupBoostAdminClient`] implementation to send the request to EngineActor's request channel.
#[derive(Debug)]
pub struct RollupBoostAdminApiClient {
    /// A channel to use to send the EngineActor requests.
    pub engine_actor_request_tx: mpsc::Sender<EngineActorRequest>,
}

impl RollupBoostAdminClient for RollupBoostAdminApiClient {
    fn set_execution_mode(
        &self,
        request: SetExecutionModeRequest,
    ) -> impl Future<Output = RpcResult<SetExecutionModeResponse>> + Send {
        let engine_actor_request_tx = self.engine_actor_request_tx.clone();
        async move {
            let (mode_tx, mode_rx) = oneshot::channel();

            engine_actor_request_tx
                .send(EngineActorRequest::RpcRequest(EngineRpcRequest::RollupBoostAdminRequest(
                    kona_rpc::RollupBoostAdminQuery::SetExecutionMode {
                        execution_mode: request.execution_mode,
                        sender: mode_tx,
                    },
                )))
                .await
                .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

            mode_rx
                .await
                .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
                .map(|_| SetExecutionModeResponse { execution_mode: request.execution_mode })
        }
    }

    fn get_execution_mode(
        &self,
    ) -> impl Future<Output = RpcResult<GetExecutionModeResponse>> + Send {
        let engine_actor_request_tx = self.engine_actor_request_tx.clone();
        async move {
            let (mode_tx, mode_rx) = oneshot::channel();

            engine_actor_request_tx
                .send(EngineActorRequest::RpcRequest(EngineRpcRequest::RollupBoostAdminRequest(
                    kona_rpc::RollupBoostAdminQuery::GetExecutionMode { sender: mode_tx },
                )))
                .await
                .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;

            mode_rx
                .await
                .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
                .map(|execution_mode| GetExecutionModeResponse { execution_mode })
        }
    }
}
