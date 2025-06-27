//! The core [`RollupNodeService`] trait
use crate::{
    AttributesBuilderConfig, DerivationContext, EngineContext, L1WatcherRpcContext, L2Finalizer,
    NetworkContext, NodeActor, NodeMode, RpcContext, RuntimeContext, SequencerContext,
    SequencerOutboundData, SupervisorActorContext, SupervisorExt,
    actors::{
        DerivationOutboundChannels, EngineOutboundData, L1WatcherRpcOutboundChannels,
        NetworkOutboundData, PipelineBuilder, RuntimeOutboundData, SupervisorOutboundData,
    },
    service::spawn_and_wait,
};
use async_trait::async_trait;
use kona_derive::{AttributesBuilder, Pipeline, SignalReceiver};
use kona_rpc::{
    NetworkRpc, OpP2PApiServer, RollupNodeApiServer, RollupRpc, RpcBuilder, RpcLauncherError,
    WsRPC, WsServer,
};
use std::fmt::Display;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// The [`RollupNodeService`] trait defines the common interface for running a rollup node.
///
/// ## Validator Mode
///
/// The rollup node, in validator mode, listens to two sources of information to sync the L2 chain:
///
/// 1. The data availability layer, with a watcher that listens for new updates. L2 inputs (L2
///    transaction batches + deposits) are then derived from the DA layer.
/// 2. The L2 sequencer, which produces unsafe L2 blocks and sends them to the network over p2p
///    gossip.
///
/// From these two sources, the node imports `unsafe` blocks from the L2 sequencer, `safe` blocks
/// from the L2 derivation pipeline into the L2 execution layer via the Engine API, and finalizes
/// `safe` blocks that it has derived when L1 finalized block updates are received.
///
/// ## Sequencer Mode
///
/// _Unimplemented - coming soon_.
///
/// ## Types
///
/// - `DataAvailabilityWatcher`: The type of [`NodeActor`] to use for the DA watcher service.
/// - `DerivationPipeline`: The type of [Pipeline] to use for the service. Can be swapped out from
///   the default implementation for the sake of plugins like Alt DA.
/// - `SupervisorExt`: The type of [`SupervisorExt`] to use for the service, which provides an
///   interface for sending events to the supervisor.
/// - `Error`: The type of error for the service's entrypoint.
#[async_trait]
pub trait RollupNodeService {
    /// The type of [`NodeActor`] to use for the DA watcher service.
    type DataAvailabilityWatcher: NodeActor<
            Error: Display,
            InboundData = L1WatcherRpcContext,
            OutboundData = L1WatcherRpcOutboundChannels,
        >;

    /// The type of derivation pipeline to use for the service.
    type DerivationPipeline: Pipeline + SignalReceiver + Send + Sync + 'static;

    /// The type of derivation actor to use for the service.
    type DerivationActor: NodeActor<
            Error: Display,
            Builder: PipelineBuilder<Pipeline = Self::DerivationPipeline>,
            InboundData = DerivationContext,
            OutboundData = DerivationOutboundChannels,
        >;

    /// The type of engine actor to use for the service.
    type EngineActor: NodeActor<Error: Display, InboundData = EngineContext, OutboundData = EngineOutboundData>;

    /// The type of network actor to use for the service.
    type NetworkActor: NodeActor<Error: Display, InboundData = NetworkContext, OutboundData = NetworkOutboundData>;

    /// The supervisor ext provider.
    type SupervisorExt: SupervisorExt + Send + Sync + 'static;

    /// The type of supervisor actor to use for the service.
    type SupervisorActor: NodeActor<
            Error: Display,
            InboundData = SupervisorActorContext,
            OutboundData = SupervisorOutboundData,
        >;

    /// The type of runtime actor to use for the service.
    type RuntimeActor: NodeActor<Error: Display, InboundData = RuntimeContext, OutboundData = RuntimeOutboundData>;

    /// The type of attributes builder to use for the sequener.
    type AttributesBuilder: AttributesBuilder + Send + Sync + 'static;

    /// The type of sequencer actor to use for the service.
    type SequencerActor: NodeActor<
            Error: Display,
            InboundData = SequencerContext,
            Builder: AttributesBuilderConfig<AB = Self::AttributesBuilder>,
            OutboundData = SequencerOutboundData,
        >;

    /// The type of rpc actor to use for the service.
    type RpcActor: NodeActor<Error: Display, InboundData = RpcContext, OutboundData = (), Builder = RpcBuilder>;

    /// The type of error for the service's entrypoint.
    type Error: From<RpcLauncherError>
        + From<jsonrpsee::server::RegisterMethodError>
        + std::fmt::Debug;

    /// The mode of operation for the node.
    fn mode(&self) -> NodeMode;

    /// Returns a DA watcher builder for the node.
    fn da_watcher_builder(&self) -> <Self::DataAvailabilityWatcher as NodeActor>::Builder;

    /// Returns a derivation builder for the node.
    fn derivation_builder(&self) -> <Self::DerivationActor as NodeActor>::Builder;

    /// Creates a network builder and the [`NetworkRpc`] for the node.
    fn network_builder(&self) -> (<Self::NetworkActor as NodeActor>::Builder, NetworkRpc);

    /// Returns a runtime builder for the node.
    fn runtime_builder(&self) -> Option<<Self::RuntimeActor as NodeActor>::Builder>;

    /// Returns an engine builder for the node.
    fn engine_builder(&self) -> <Self::EngineActor as NodeActor>::Builder;

    /// Returns the [`RpcBuilder`] for the node.
    fn rpc_builder(&self) -> RpcBuilder;

    /// Returns the sequencer builder for the node.
    fn sequencer_builder(&self) -> <Self::SequencerActor as NodeActor>::Builder;

    /// Creates a new [`Self::SupervisorExt`] to be used in the supervisor rpc actor.
    async fn supervisor_ext(&self) -> Option<Self::SupervisorExt>;

    /// Starts the rollup node service.
    async fn start(&self) -> Result<(), Self::Error> {
        // Create a global cancellation token for graceful shutdown of tasks.
        let cancellation = CancellationToken::new();

        // Create the DA watcher actor.
        let da_watcher_builder = self.da_watcher_builder();
        let (
            L1WatcherRpcOutboundChannels { latest_head, latest_finalized, block_signer_sender },
            da_watcher,
        ) = Self::DataAvailabilityWatcher::build(da_watcher_builder);

        // Create the derivation actor.
        let derivation_builder = self.derivation_builder();
        let (DerivationOutboundChannels { attributes_out }, derivation) =
            Self::DerivationActor::build(derivation_builder);

        // TODO: get the supervisor ext.
        // TODO: use the supervisor ext to create the supervisor actor.
        // let supervisor_ext = self.supervisor_ext();
        // let supervisor_rpx = SupervisorActor::new(
        //
        // )

        // Create the runtime actor.
        let (runtime_config, runtime) = self
            .runtime_builder()
            .map(|builder| {
                let (RuntimeOutboundData { runtime_config }, runtime) =
                    Self::RuntimeActor::build(builder);
                (runtime_config, runtime)
            })
            .unzip();

        // Create the engine actor.
        let engine_builder = self.engine_builder();
        let (
            EngineOutboundData {
                reset_request_tx,
                engine_l2_safe_head_rx,
                sync_complete_rx,
                derivation_signal_rx,
            },
            engine,
        ) = Self::EngineActor::build(engine_builder);

        // Create the p2p actor.
        let (network_builder, p2p_rpc_module) = self.network_builder();
        let (NetworkOutboundData { unsafe_block }, network) =
            Self::NetworkActor::build(network_builder);

        // Create the RPC server actor.
        let (engine_query_recv, l1_watcher_queries_recv, (_, rpc)) = {
            let mut rpc_builder = self.rpc_builder().with_healthz()?;

            rpc_builder.merge(p2p_rpc_module.into_rpc())?;

            // Create context for communication between actors.
            let (l1_watcher_queries_sender, l1_watcher_queries_recv) = mpsc::channel(1024);
            let (engine_query_sender, engine_query_recv) = mpsc::channel(1024);
            let rollup_rpc = RollupRpc::new(engine_query_sender.clone(), l1_watcher_queries_sender);
            rpc_builder.merge(rollup_rpc.into_rpc())?;

            if rpc_builder.ws_enabled() {
                rpc_builder
                    .merge(WsRPC::new(engine_query_sender).into_rpc())
                    .map_err(Self::Error::from)?;
            }

            (engine_query_recv, l1_watcher_queries_recv, Self::RpcActor::build(rpc_builder))
        };

        let (_, sequencer) = Self::SequencerActor::build(self.sequencer_builder());

        let network_context =
            NetworkContext { signer: block_signer_sender, cancellation: cancellation.clone() };

        let da_watcher_context = L1WatcherRpcContext {
            inbound_queries: l1_watcher_queries_recv,
            cancellation: cancellation.clone(),
        };

        let derivation_context = DerivationContext {
            reset_request_tx,
            l1_head_updates: latest_head,
            engine_l2_safe_head: engine_l2_safe_head_rx.clone(),
            el_sync_complete_rx: sync_complete_rx,
            derivation_signal_rx,
            cancellation: cancellation.clone(),
        };

        let engine_context = EngineContext {
            runtime_config_rx: runtime_config,
            attributes_rx: attributes_out,
            unsafe_block_rx: unsafe_block,
            inbound_queries: engine_query_recv,
            cancellation: cancellation.clone(),
            finalizer: L2Finalizer::new(latest_finalized),
        };

        let rpc_context = RpcContext { cancellation: cancellation.clone() };

        let sequencer_context = SequencerContext {
            latest_payload_rx: None,
            unsafe_head: engine_l2_safe_head_rx,
            cancellation: cancellation.clone(),
        };

        spawn_and_wait!(
            cancellation,
            actors = [
                runtime.map(|r| (r, RuntimeContext { cancellation: cancellation.clone() })),
                Some((network, network_context)),
                Some((da_watcher, da_watcher_context)),
                Some((derivation, derivation_context)),
                Some((engine, engine_context)),
                Some((rpc, rpc_context)),
                (self.mode() == NodeMode::Sequencer).then_some((sequencer, sequencer_context))
            ]
        );
        Ok(())
    }
}
