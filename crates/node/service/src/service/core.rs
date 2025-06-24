//! The core [`RollupNodeService`] trait

use super::NodeMode;
use crate::{
    DerivationActor, EngineActor, EngineLauncher, NetworkActor, NodeActor, RpcActor,
    RuntimeLauncher, SupervisorExt, service::spawn_and_wait,
};
use alloy_primitives::Address;
use async_trait::async_trait;
use kona_derive::{Pipeline, SignalReceiver};
use kona_genesis::RollupConfig;
use kona_p2p::Network;
use kona_protocol::{BlockInfo, L2BlockInfo};
use kona_rpc::{
    L1WatcherQueries, NetworkRpc, OpP2PApiServer, RollupNodeApiServer, RollupRpc, RpcLauncher,
    RpcLauncherError, WsRPC, WsServer,
};
use std::fmt::Display;
use tokio::sync::{mpsc, oneshot, watch};
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
    type DataAvailabilityWatcher: NodeActor<Error: Display> + Send + Sync + 'static;
    /// The type of derivation pipeline to use for the service.
    type DerivationPipeline: Pipeline + SignalReceiver + Send + Sync + 'static;
    /// The supervisor ext provider.
    type SupervisorExt: SupervisorExt + Send + Sync + 'static;
    /// The type of error for the service's entrypoint.
    type Error: From<RpcLauncherError>
        + From<jsonrpsee::server::RegisterMethodError>
        + std::fmt::Debug;

    /// Returns the [`NodeMode`] of the service.
    fn mode(&self) -> NodeMode;

    /// Returns a reference to the rollup node's [`RollupConfig`].
    fn config(&self) -> &RollupConfig;

    /// Creates a new [`NodeActor`] instance that watches the data availability layer. The
    /// `cancellation` token is used to gracefully shut down the actor.
    fn new_da_watcher(
        &self,
        head_updates: watch::Sender<Option<BlockInfo>>,
        finalized_updates: watch::Sender<Option<BlockInfo>>,
        block_signer_tx: mpsc::Sender<Address>,
        cancellation: CancellationToken,
        l1_watcher_inbound_queries: mpsc::Receiver<L1WatcherQueries>,
    ) -> (Self::DataAvailabilityWatcher, <Self::DataAvailabilityWatcher as NodeActor>::Context);

    /// Creates a new instance of the [`Pipeline`] and initializes it. Returns the starting L2
    /// forkchoice state and the initialized derivation pipeline.
    async fn init_derivation(&self) -> Result<Self::DerivationPipeline, Self::Error>;

    /// Creates a new instance of the [`Network`].
    async fn init_network(&self) -> Result<(Network, NetworkRpc), Self::Error>;

    /// Creates a new [`Self::SupervisorExt`] to be used in the supervisor rpc actor.
    async fn supervisor_ext(&self) -> Option<Self::SupervisorExt>;

    /// Returns the [`RuntimeLauncher`] for the node.
    fn runtime(&self) -> RuntimeLauncher;

    /// Returns the [`EngineLauncher`]
    fn engine(&self) -> EngineLauncher;

    /// Returns the [`RpcLauncher`] for the node.
    fn rpc(&self) -> RpcLauncher;

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

        // Create context for communication between actors.
        let (sync_complete_tx, sync_complete_rx) = oneshot::channel();
        let (derived_payload_tx, derived_payload_rx) = mpsc::channel(16);
        let (runtime_config_tx, runtime_config_rx) = mpsc::channel(16);
        let (derivation_signal_tx, derivation_signal_rx) = mpsc::channel(16);
        let (reset_request_tx, reset_request_rx) = mpsc::channel(16);
        let (block_signer_tx, block_signer_rx) = mpsc::channel(16);
        let (unsafe_block_tx, unsafe_block_rx) = mpsc::channel(1024);
        let (l1_watcher_queries_sender, l1_watcher_queries_recv) = mpsc::channel(1024);
        let (engine_query_sender, engine_query_recv) = mpsc::channel(1024);
        let (head_updates_tx, head_updates_rx) = watch::channel(None);
        let (finalized_updates_tx, finalized_updates_rx) = watch::channel(None);
        let (engine_l2_safe_tx, engine_l2_safe_rx) = watch::channel(L2BlockInfo::default());

        // Create the DA watcher actor.
        let da_watcher = self.new_da_watcher(
            head_updates_tx,
            finalized_updates_tx,
            block_signer_tx,
            cancellation.clone(),
            l1_watcher_queries_recv,
        );

        // Create the derivation actor.
        let derivation_pipeline = self.init_derivation().await?;
        let derivation = DerivationActor::new(
            derivation_pipeline,
            engine_l2_safe_rx,
            sync_complete_rx,
            derivation_signal_rx,
            head_updates_rx,
            derived_payload_tx,
            reset_request_tx,
            cancellation.clone(),
        );

        // TODO: get the supervisor ext.
        // TODO: use the supervisor ext to create the supervisor actor.
        // let supervisor_ext = self.supervisor_ext();
        // let supervisor_rpx = SupervisorActor::new(
        //
        // )

        // Create the runtime configuration actor.
        let runtime = self
            .runtime()
            .with_tx(runtime_config_tx)
            .with_cancellation(cancellation.clone())
            .launch();

        // Create the engine actor.
        let engine_launcher = self.engine();
        let client = engine_launcher.client();
        let engine_task_queue = engine_launcher.launch();
        let engine = EngineActor::new(
            std::sync::Arc::new(self.config().clone()),
            client,
            engine_task_queue,
            engine_l2_safe_tx,
            sync_complete_tx,
            derivation_signal_tx,
            runtime_config_rx,
            derived_payload_rx,
            unsafe_block_rx,
            reset_request_rx,
            finalized_updates_rx,
            Some(engine_query_recv),
            cancellation.clone(),
        );

        // Create the p2p actor.
        let (p2p_rpc_module, network) = {
            let (driver, module) = self.init_network().await?;
            let actor =
                NetworkActor::new(driver, unsafe_block_tx, block_signer_rx, cancellation.clone());

            (module, actor)
        };

        // Create the RPC server actor.
        let rpc = {
            let mut rpc_launcher = self.rpc().with_healthz()?;

            rpc_launcher.merge(p2p_rpc_module.into_rpc())?;

            let rollup_rpc = RollupRpc::new(engine_query_sender.clone(), l1_watcher_queries_sender);
            rpc_launcher.merge(rollup_rpc.into_rpc())?;

            if rpc_launcher.ws_enabled() {
                rpc_launcher
                    .merge(WsRPC::new(engine_query_sender).into_rpc())
                    .map_err(Self::Error::from)?;
            }

            RpcActor::new(rpc_launcher, cancellation.clone())
        };

        spawn_and_wait!(
            cancellation,
            actors = [
                runtime,
                Some(network),
                Some(da_watcher),
                Some(derivation),
                Some(engine),
                Some(rpc)
            ]
        );
        Ok(())
    }
}
