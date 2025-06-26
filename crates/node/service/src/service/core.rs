//! The core [`RollupNodeService`] trait

use super::NodeMode;
use crate::{
    DerivationContext, DerivationState, EngineContext, EngineLauncher, L1WatcherRpcContext,
    L2Finalizer, NetworkContext, NodeActor, RpcContext, RuntimeContext, SequencerActorState,
    SequencerContext, SequencerOutboundData, SupervisorActorContext, SupervisorExt,
    actors::{
        DerivationOutboundChannels, EngineActorState, EngineOutboundData,
        L1WatcherRpcOutboundChannels, L1WatcherRpcState, NetworkOutboundData, RuntimeOutboundData,
        RuntimeState, SupervisorOutboundData,
    },
    service::spawn_and_wait,
};
use alloy_provider::RootProvider;
use async_trait::async_trait;
use kona_derive::{AttributesBuilder, Pipeline, SignalReceiver};
use kona_genesis::RollupConfig;
use kona_p2p::Network;
use kona_rpc::{
    NetworkRpc, OpP2PApiServer, RollupNodeApiServer, RollupRpc, RpcLauncher, RpcLauncherError,
    WsRPC, WsServer,
};
use std::{fmt::Display, sync::Arc};
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
            State = L1WatcherRpcState,
            OutboundData = L1WatcherRpcOutboundChannels,
        >;

    /// The type of derivation pipeline to use for the service.
    type DerivationPipeline: Pipeline + SignalReceiver + Send + Sync + 'static;

    /// The type of attributes builder to use for the sequener.
    type AttributesBuilder: AttributesBuilder + Send + Sync + 'static;

    /// The type of derivation actor to use for the service.
    type DerivationActor: NodeActor<
            Error: Display,
            InboundData = DerivationContext,
            State = DerivationState<Self::DerivationPipeline>,
            OutboundData = DerivationOutboundChannels,
        >;

    /// The type of engine actor to use for the service.
    type EngineActor: NodeActor<
            Error: Display,
            InboundData = EngineContext,
            State = EngineActorState,
            OutboundData = EngineOutboundData,
        >;

    /// The type of network actor to use for the service.
    type NetworkActor: NodeActor<
            Error: Display,
            InboundData = NetworkContext,
            State = Network,
            OutboundData = NetworkOutboundData,
        >;

    /// The supervisor ext provider.
    type SupervisorExt: SupervisorExt + Send + Sync + 'static;

    /// The type of supervisor actor to use for the service.
    type SupervisorActor: NodeActor<
            Error: Display,
            InboundData = SupervisorActorContext,
            State = Self::SupervisorExt,
            OutboundData = SupervisorOutboundData,
        >;

    /// The type of runtime actor to use for the service.
    type RuntimeActor: NodeActor<
            Error: Display,
            InboundData = RuntimeContext,
            State = RuntimeState,
            OutboundData = RuntimeOutboundData,
        >;

    /// The type of sequencer actor to use for the service.
    type SequencerActor: NodeActor<
            Error: Display,
            InboundData = SequencerContext,
            State = SequencerActorState<Self::AttributesBuilder>,
            OutboundData = SequencerOutboundData,
        >;

    /// The type of rpc actor to use for the service.
    type RpcActor: NodeActor<Error: Display, InboundData = RpcContext, State = RpcLauncher, OutboundData = ()>;

    /// The type of error for the service's entrypoint.
    type Error: From<RpcLauncherError>
        + From<jsonrpsee::server::RegisterMethodError>
        + std::fmt::Debug;

    /// Returns the [`NodeMode`] of the service.
    fn mode(&self) -> NodeMode;

    /// Returns a reference to the rollup node's [`RollupConfig`].
    fn config(&self) -> Arc<RollupConfig>;

    /// Returns the [`RootProvider`] for the L1 chain.
    fn l1_provider(&self) -> RootProvider;

    /// Creates a new instance of the [`Pipeline`] and initializes it. Returns the starting L2
    /// forkchoice state and the initialized derivation pipeline.
    async fn init_derivation(&self) -> Result<Self::DerivationPipeline, Self::Error>;

    /// Creates a new instance of the [`Network`].
    async fn init_network(&self) -> Result<(Network, NetworkRpc), Self::Error>;

    /// Creates a new [`Self::SupervisorExt`] to be used in the supervisor rpc actor.
    async fn supervisor_ext(&self) -> Option<Self::SupervisorExt>;

    /// Returns the [`RuntimeState`] for the node.
    fn runtime(&self) -> Option<&RuntimeState>;

    /// Returns the [`EngineLauncher`]
    fn engine(&self) -> EngineLauncher;

    /// Returns the [`RpcLauncher`] for the node.
    fn rpc(&self) -> RpcLauncher;

    /// Returns the initial [`SequencerActorState`].
    fn sequencer_state(&self) -> SequencerActorState<Self::AttributesBuilder>;

    /// Starts the rollup node service.
    async fn start(&self) -> Result<(), Self::Error> {
        info!(
            target: "rollup_node",
            mode = %self.mode(),
            chain_id = self.config().l2_chain_id,
            "Starting rollup node services"
        );
        for hf in self.config().hardforks.to_string().lines() {
            info!(target: "rollup_node", "{hf}");
        }

        // Create a global cancellation token for graceful shutdown of tasks.
        let cancellation = CancellationToken::new();

        // Create the DA watcher actor.
        let (
            L1WatcherRpcOutboundChannels { latest_head, latest_finalized, block_signer_sender },
            da_watcher,
        ) = Self::DataAvailabilityWatcher::build(L1WatcherRpcState {
            rollup: self.config(),
            l1_provider: self.l1_provider(),
        });

        // Create the derivation actor.
        let derivation_pipeline = self.init_derivation().await?;
        let (DerivationOutboundChannels { attributes_out, reset_request_tx }, derivation) =
            Self::DerivationActor::build(DerivationState::new(derivation_pipeline));

        // TODO: get the supervisor ext.
        // TODO: use the supervisor ext to create the supervisor actor.
        // let supervisor_ext = self.supervisor_ext();
        // let supervisor_rpx = SupervisorActor::new(
        //
        // )

        // Create the runtime configuration actor.
        let (runtime_config, runtime) = self
            .runtime()
            .map(|state| {
                let (RuntimeOutboundData { runtime_config }, runtime) =
                    Self::RuntimeActor::build(state.clone());
                (runtime_config, runtime)
            })
            .unzip();

        // Create the engine actor.
        let engine_launcher = self.engine();
        let client = engine_launcher.client();
        let engine_task_queue = engine_launcher.launch();
        let (
            EngineOutboundData { engine_l2_safe_head_rx, sync_complete_rx, derivation_signal_rx },
            engine,
        ) = Self::EngineActor::build(EngineActorState {
            rollup: self.config(),
            client: client.clone().into(),
            engine: engine_task_queue,
        });

        // Create the p2p actor.
        let (driver, p2p_rpc_module) = self.init_network().await?;
        let (NetworkOutboundData { unsafe_block }, network) = Self::NetworkActor::build(driver);

        // Create the RPC server actor.
        let (engine_query_recv, l1_watcher_queries_recv, (_, rpc)) = {
            let mut rpc_launcher = self.rpc().with_healthz()?;

            rpc_launcher.merge(p2p_rpc_module.into_rpc())?;

            // Create context for communication between actors.
            let (l1_watcher_queries_sender, l1_watcher_queries_recv) = mpsc::channel(1024);
            let (engine_query_sender, engine_query_recv) = mpsc::channel(1024);
            let rollup_rpc = RollupRpc::new(engine_query_sender.clone(), l1_watcher_queries_sender);
            rpc_launcher.merge(rollup_rpc.into_rpc())?;

            if rpc_launcher.ws_enabled() {
                rpc_launcher
                    .merge(WsRPC::new(engine_query_sender).into_rpc())
                    .map_err(Self::Error::from)?;
            }

            (engine_query_recv, l1_watcher_queries_recv, Self::RpcActor::build(rpc_launcher))
        };

        let (_, sequencer) = Self::SequencerActor::build(self.sequencer_state());

        let network_context =
            NetworkContext { signer: block_signer_sender, cancellation: cancellation.clone() };

        let da_watcher_context = L1WatcherRpcContext {
            inbound_queries: l1_watcher_queries_recv,
            cancellation: cancellation.clone(),
        };

        let derivation_context = DerivationContext {
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
            reset_request_rx: reset_request_tx,
            inbound_queries: engine_query_recv,
            cancellation: cancellation.clone(),
            finalizer: L2Finalizer::new(latest_finalized, client.into()),
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
