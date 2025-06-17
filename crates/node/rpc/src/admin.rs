//! Admin RPC Module

use crate::{AdminApiServer, NetworkRpc};
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorCode, ErrorObject},
};
use kona_p2p::P2pRpcRequest;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;

#[async_trait]
impl AdminApiServer for NetworkRpc {
    async fn admin_post_unsafe_payload(
        &self,
        payload: OpExecutionPayloadEnvelope,
    ) -> RpcResult<()> {
        kona_macros::inc!(gauge, kona_p2p::Metrics::RPC_CALLS, "method" => "admin_postUnsafePayload");
        self.sender
            .send(P2pRpcRequest::PostUnsafePayload { payload })
            .await
            .map_err(|_| ErrorObject::from(ErrorCode::InternalError))
    }
}
