//! An Engine API Client.

use crate::Metrics;
use alloy_eips::eip1898::BlockNumberOrTag;
use alloy_network::{AnyNetwork, Network};
use alloy_primitives::{B256, BlockHash, Bytes};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_engine::{
    ClientVersionV1, ExecutionPayloadBodiesV1, ExecutionPayloadEnvelopeV2, ExecutionPayloadInputV2,
    ExecutionPayloadV3, ForkchoiceState, ForkchoiceUpdated, JwtSecret, PayloadId, PayloadStatus,
};
use alloy_rpc_types_eth::Block;
use alloy_transport::{RpcError, TransportErrorKind, TransportResult};
use alloy_transport_http::{
    AuthLayer, AuthService, Http, HyperClient,
    hyper_util::{
        client::legacy::{Client, connect::HttpConnector},
        rt::TokioExecutor,
    },
};
use derive_more::Deref;
use http_body_util::Full;
use kona_genesis::RollupConfig;
use kona_protocol::{FromBlockError, L2BlockInfo};
use op_alloy_network::Optimism;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types::Transaction;
use op_alloy_rpc_types_engine::{
    OpExecutionPayloadEnvelopeV3, OpExecutionPayloadEnvelopeV4, OpExecutionPayloadV4,
    OpPayloadAttributes, ProtocolVersion,
};
use std::{sync::Arc, time::Instant};
use thiserror::Error;
use tower::ServiceBuilder;
use url::Url;

/// An error that occurred in the [`EngineClient`].
#[derive(Error, Debug)]
pub enum EngineClientError {
    /// An RPC error occurred
    #[error("An RPC error occurred: {0}")]
    RpcError(#[from] RpcError<TransportErrorKind>),

    /// An error occurred while decoding the payload
    #[error("An error occurred while decoding the payload: {0}")]
    BlockInfoDecodeError(#[from] FromBlockError),
}
/// A Hyper HTTP client with a JWT authentication layer.
type HyperAuthClient<B = Full<Bytes>> = HyperClient<B, AuthService<Client<HttpConnector, B>>>;

/// An external engine api client
#[derive(Debug, Deref, Clone)]
pub struct EngineClient {
    /// The L2 engine provider.
    #[deref]
    engine: RootProvider<AnyNetwork>,
    /// The L2 chain provider.
    l2_provider: RootProvider<Optimism>,
    /// The L1 chain provider.
    l1_provider: RootProvider,
    /// The [RollupConfig] for the chain used to timestamp which version of the engine api to use.
    cfg: Arc<RollupConfig>,
}

impl EngineClient {
    /// Creates a new RPC client for the given address and JWT secret.
    fn rpc_client<T: Network>(addr: Url, jwt: JwtSecret) -> RootProvider<T> {
        let hyper_client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();
        let auth_layer = AuthLayer::new(jwt);
        let service = ServiceBuilder::new().layer(auth_layer).service(hyper_client);
        let layer_transport = HyperClient::with_service(service);

        let http_hyper = Http::with_client(layer_transport, addr);
        let rpc_client = RpcClient::new(http_hyper, false);
        RootProvider::<T>::new(rpc_client)
    }

    /// Creates a new [`EngineClient`] from the provided [Url] and [JwtSecret].
    pub fn new_http(
        engine: Url,
        l2_rpc: Url,
        l1_rpc: Url,
        cfg: Arc<RollupConfig>,
        jwt: JwtSecret,
    ) -> Self {
        let engine = Self::rpc_client::<AnyNetwork>(engine, jwt);
        let l2_provider = Self::rpc_client::<Optimism>(l2_rpc, jwt);
        let l1_provider = RootProvider::new_http(l1_rpc);

        Self { engine, l2_provider, l1_provider, cfg }
    }

    /// Returns a reference to the inner L2 [`RootProvider`].
    pub const fn l2_provider(&self) -> &RootProvider<Optimism> {
        &self.l2_provider
    }

    /// Returns a reference to the inner L1 [`RootProvider`].
    pub const fn l1_provider(&self) -> &RootProvider {
        &self.l1_provider
    }

    /// Returns a reference to the inner [`RollupConfig`].
    pub fn cfg(&self) -> &RollupConfig {
        self.cfg.as_ref()
    }

    /// Fetches the [`Block<T>`] for the given [`BlockNumberOrTag`].
    pub async fn l2_block_by_label(
        &self,
        numtag: BlockNumberOrTag,
    ) -> Result<Option<Block<Transaction>>, EngineClientError> {
        Ok(<RootProvider<Optimism>>::get_block_by_number(&self.l2_provider, numtag).full().await?)
    }

    /// Fetches the [L2BlockInfo] by [BlockNumberOrTag].
    pub async fn l2_block_info_by_label(
        &self,
        numtag: BlockNumberOrTag,
    ) -> Result<Option<L2BlockInfo>, EngineClientError> {
        let block =
            <RootProvider<Optimism>>::get_block_by_number(&self.l2_provider, numtag).full().await?;
        let Some(block) = block else {
            return Ok(None);
        };
        Ok(Some(L2BlockInfo::from_block_and_genesis(&block.into_consensus(), &self.cfg.genesis)?))
    }
}

#[async_trait::async_trait]
impl OpEngineApi<AnyNetwork, Http<HyperAuthClient>> for EngineClient {
    async fn new_payload_v2(
        &self,
        payload: ExecutionPayloadInputV2,
    ) -> TransportResult<PayloadStatus> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::new_payload_v2(&self.engine, payload);

        record_call_time(call, Metrics::NEW_PAYLOAD_METHOD).await
    }

    async fn new_payload_v3(
        &self,
        payload: ExecutionPayloadV3,
        parent_beacon_block_root: B256,
    ) -> TransportResult<PayloadStatus> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::new_payload_v3(&self.engine, payload, parent_beacon_block_root);

        record_call_time(call, Metrics::NEW_PAYLOAD_METHOD).await
    }

    async fn new_payload_v4(
        &self,
        payload: OpExecutionPayloadV4,
        parent_beacon_block_root: B256,
    ) -> TransportResult<PayloadStatus> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::new_payload_v4(&self.engine, payload, parent_beacon_block_root);

        record_call_time(call, Metrics::NEW_PAYLOAD_METHOD).await
    }

    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> TransportResult<ForkchoiceUpdated> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::fork_choice_updated_v2(&self.engine, fork_choice_state, payload_attributes);

        record_call_time(call, Metrics::FORKCHOICE_UPDATE_METHOD).await
    }

    async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> TransportResult<ForkchoiceUpdated> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::fork_choice_updated_v3(&self.engine, fork_choice_state, payload_attributes);

        record_call_time(call, Metrics::FORKCHOICE_UPDATE_METHOD).await
    }

    async fn get_payload_v2(
        &self,
        payload_id: PayloadId,
    ) -> TransportResult<ExecutionPayloadEnvelopeV2> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_v2(&self.engine, payload_id);

        record_call_time(call, Metrics::GET_PAYLOAD_METHOD).await
    }

    async fn get_payload_v3(
        &self,
        payload_id: PayloadId,
    ) -> TransportResult<OpExecutionPayloadEnvelopeV3> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_v3(&self.engine, payload_id);

        record_call_time(call, Metrics::GET_PAYLOAD_METHOD).await
    }

    async fn get_payload_v4(
        &self,
        payload_id: PayloadId,
    ) -> TransportResult<OpExecutionPayloadEnvelopeV4> {
        let call = <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::get_payload_v4(&self.engine, payload_id);

        record_call_time(call, Metrics::GET_PAYLOAD_METHOD).await
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

    async fn signal_superchain_v1(
        &self,
        recommended: ProtocolVersion,
        required: ProtocolVersion,
    ) -> TransportResult<ProtocolVersion> {
        <RootProvider<AnyNetwork> as OpEngineApi<
            AnyNetwork,
            Http<HyperAuthClient>,
        >>::signal_superchain_v1(&self.engine, recommended, required).await
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

/// Wrapper to record the time taken for a call to the engine API and log the result as a metric.
async fn record_call_time<T>(
    f: impl Future<Output = TransportResult<T>>,
    metric_label: &'static str,
) -> TransportResult<T> {
    // Await on the future and track its duration.
    let start = Instant::now();
    let result = f.await?;
    let duration = start.elapsed();

    // Record the call duration.
    kona_macros::record!(
        histogram,
        Metrics::ENGINE_METHOD_REQUEST_DURATION,
        "method",
        metric_label,
        duration.as_secs_f64()
    );
    Ok(result)
}
