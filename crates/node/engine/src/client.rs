//! An Engine API Client.

use alloy_eips::eip1898::BlockNumberOrTag;
use alloy_network::AnyNetwork;
use alloy_primitives::{B256, BlockHash, Bytes};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_engine::{
    ClientVersionV1, ExecutionPayloadBodiesV1, ExecutionPayloadEnvelopeV2, ExecutionPayloadInputV2,
    ExecutionPayloadV3, ForkchoiceState, ForkchoiceUpdated, JwtSecret, PayloadId, PayloadStatus,
};
use alloy_transport::TransportResult;
use alloy_transport_http::{
    AuthLayer, AuthService, Http, HyperClient,
    hyper_util::{
        client::legacy::{Client, connect::HttpConnector},
        rt::TokioExecutor,
    },
};
use anyhow::Result;
use derive_more::Deref;
use http_body_util::Full;
use op_alloy_network::Optimism;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::{
    OpExecutionPayloadEnvelopeV3, OpExecutionPayloadEnvelopeV4, OpPayloadAttributes,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use url::Url;

use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;

/// A Hyper HTTP client with a JWT authentication layer.
type HyperAuthClient<B = Full<Bytes>> = HyperClient<B, AuthService<Client<HttpConnector, B>>>;

/// An external engine api client
#[derive(Debug, Deref, Clone)]
pub struct EngineClient {
    /// The L2 engine provider.
    #[deref]
    engine: RootProvider<AnyNetwork>,
    /// The L2 chain provider.
    rpc: RootProvider<Optimism>,
    /// The [RollupConfig] for the chain used to timestamp which version of the engine api to use.
    cfg: Arc<RollupConfig>,
}

impl EngineClient {
    /// Creates a new [`EngineClient`] from the provided [Url] and [JwtSecret].
    pub fn new_http(engine: Url, rpc: Url, cfg: Arc<RollupConfig>, jwt: JwtSecret) -> Self {
        let hyper_client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();

        let auth_layer = AuthLayer::new(jwt);
        let service = ServiceBuilder::new().layer(auth_layer).service(hyper_client);

        let layer_transport = HyperClient::with_service(service);
        let http_hyper = Http::with_client(layer_transport, engine);
        let rpc_client = RpcClient::new(http_hyper, true);
        let engine = RootProvider::<AnyNetwork>::new(rpc_client);

        let rpc = RootProvider::<Optimism>::new_http(rpc);
        Self { engine, rpc, cfg }
    }

    /// Fetches the [L2BlockInfo] by [BlockNumberOrTag].
    pub async fn l2_block_info_by_label(
        &self,
        numtag: BlockNumberOrTag,
    ) -> Result<Option<L2BlockInfo>> {
        let block = <RootProvider<Optimism>>::get_block_by_number(&self.rpc, numtag)
            .full()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        match block.map(|b| b.into_consensus()) {
            Some(block) => {
                Ok(Some(L2BlockInfo::from_block_and_genesis(&block, &self.cfg.genesis)?))
            }
            None => Ok(None),
        }
    }
}

#[async_trait::async_trait]
impl OpEngineApi<AnyNetwork, Http<HyperAuthClient>> for EngineClient {
    async fn new_payload_v2(
        &self,
        payload: ExecutionPayloadInputV2,
    ) -> TransportResult<PayloadStatus> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::new_payload_v2(&self.engine, payload).await
    }

    async fn new_payload_v3(
        &self,
        payload: ExecutionPayloadV3,
        parent_beacon_block_root: B256,
    ) -> TransportResult<PayloadStatus> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::new_payload_v3(&self.engine, payload, parent_beacon_block_root).await
    }

    async fn new_payload_v4(
        &self,
        payload: ExecutionPayloadV3,
        parent_beacon_block_root: B256,
    ) -> TransportResult<PayloadStatus> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::new_payload_v4(&self.engine, payload, parent_beacon_block_root).await
    }

    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> TransportResult<ForkchoiceUpdated> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::fork_choice_updated_v2(&self.engine, fork_choice_state, payload_attributes).await
    }

    async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> TransportResult<ForkchoiceUpdated> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::fork_choice_updated_v3(&self.engine, fork_choice_state, payload_attributes).await
    }

    async fn get_payload_v2(
        &self,
        payload_id: PayloadId,
    ) -> TransportResult<ExecutionPayloadEnvelopeV2> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_v2(&self.engine, payload_id).await
    }

    async fn get_payload_v3(
        &self,
        payload_id: PayloadId,
    ) -> TransportResult<OpExecutionPayloadEnvelopeV3> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_v3(&self.engine, payload_id).await
    }

    async fn get_payload_v4(
        &self,
        payload_id: PayloadId,
    ) -> TransportResult<OpExecutionPayloadEnvelopeV4> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_v4(&self.engine, payload_id).await
    }

    async fn get_payload_bodies_by_hash_v1(
        &self,
        block_hashes: Vec<BlockHash>,
    ) -> TransportResult<ExecutionPayloadBodiesV1> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_bodies_by_hash_v1(&self.engine, block_hashes).await
    }

    async fn get_payload_bodies_by_range_v1(
        &self,
        start: u64,
        count: u64,
    ) -> TransportResult<ExecutionPayloadBodiesV1> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_bodies_by_range_v1(&self.engine, start, count).await
    }

    async fn get_client_version_v1(
        &self,
        client_version: ClientVersionV1,
    ) -> TransportResult<Vec<ClientVersionV1>> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_client_version_v1(&self.engine, client_version).await
    }

    async fn exchange_capabilities(
        &self,
        capabilities: Vec<String>,
    ) -> TransportResult<Vec<String>> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::exchange_capabilities(&self.engine, capabilities).await
    }
}
