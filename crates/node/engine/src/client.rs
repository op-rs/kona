//! An Engine API Client.

use alloy_eips::eip1898::BlockNumberOrTag;
use alloy_network::{AnyNetwork, Network};
use alloy_primitives::{B256, BlockHash, Bytes};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_engine::{
    ClientVersionV1, ExecutionPayloadBodiesV1, ExecutionPayloadEnvelopeV2, ExecutionPayloadInputV2,
    ExecutionPayloadV3, ForkchoiceState, ForkchoiceUpdated, JwtSecret, PayloadId, PayloadStatus,
};
use alloy_rpc_types_eth::{Block, SyncStatus};
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
use op_alloy_network::Optimism;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types::Transaction;
use op_alloy_rpc_types_engine::{
    OpExecutionPayloadEnvelopeV3, OpExecutionPayloadEnvelopeV4, OpExecutionPayloadV4,
    OpPayloadAttributes, ProtocolVersion, SuperchainSignal,
};
use std::sync::Arc;
use thiserror::Error;
use tower::ServiceBuilder;
use url::Url;

use kona_genesis::RollupConfig;
use kona_protocol::{FromBlockError, L2BlockInfo};

/// An error that occured in the [EngineClient].
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
    rpc: RootProvider<Optimism>,
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
    pub fn new_http(engine: Url, rpc: Url, cfg: Arc<RollupConfig>, jwt: JwtSecret) -> Self {
        let engine = Self::rpc_client::<AnyNetwork>(engine, jwt);
        let rpc = Self::rpc_client::<Optimism>(rpc, jwt);

        Self { engine, rpc, cfg }
    }

    /// Returns the [`SyncStatus`] of the engine.
    pub async fn syncing(&self) -> Result<SyncStatus, EngineClientError> {
        let status = <RootProvider<AnyNetwork>>::syncing(&self.engine).await?;
        Ok(status)
    }

    /// Signals the engine with the given [`SuperchainSignal`].
    ///
    /// This is an optional extension to the Engine API.
    ///
    /// Signals superchain information to the Engine: V1 signals which protocol version is
    /// recommended and required.
    ///
    /// # Returns
    ///
    /// - *ProtocolVersion*: The latest supported OP-Stack protocol version of the execution engine.
    ///
    /// See: <https://specs.optimism.io/protocol/exec-engine.html#engine_signalsuperchainv1>
    pub async fn signal(
        &self,
        signal: SuperchainSignal,
    ) -> Result<ProtocolVersion, EngineClientError> {
        let version = self.engine.client().request("engine_signalSuperchainV1", (signal,)).await?;
        Ok(version)
    }

    /// Fetches the [`Block<T>`] for the given [`BlockNumberOrTag`].
    pub async fn l2_block_by_label(
        &self,
        numtag: BlockNumberOrTag,
    ) -> Result<Option<Block<Transaction>>, EngineClientError> {
        Ok(<RootProvider<Optimism>>::get_block_by_number(&self.rpc, numtag).full().await?)
    }

    /// Fetches the [L2BlockInfo] by [BlockNumberOrTag].
    pub async fn l2_block_info_by_label(
        &self,
        numtag: BlockNumberOrTag,
    ) -> Result<Option<L2BlockInfo>, EngineClientError> {
        let block = <RootProvider<Optimism>>::get_block_by_number(&self.rpc, numtag).full().await?;
        let Some(block) = block else {
            return Ok(None);
        };
        Ok(Some(L2BlockInfo::from_rpc_block_and_genesis(block, &self.cfg.genesis)?))
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
        payload: OpExecutionPayloadV4,
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
