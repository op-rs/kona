//! [ValidatorNodeService] trait.

use crate::{
    DerivationActor, EngineActor, EngineLauncher, NetworkActor, NodeActor, RpcActor,
    RuntimeLauncher, service::spawn_and_wait,
};
use alloy_primitives::Address;
use async_trait::async_trait;
use kona_derive::traits::{Pipeline, SignalReceiver};
use kona_engine::EngineStateBuilderError;
use kona_genesis::RollupConfig;
use kona_p2p::Network;
use kona_protocol::{BlockInfo, L2BlockInfo};
use kona_rpc::{
    L1WatcherQueries, NetworkRpc, OpP2PApiServer, RollupNodeApiServer, RollupRpc, RpcLauncher,
    RpcLauncherError,
};
use kona_sources::L2ForkchoiceState;
use std::fmt::Display;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_util::sync::CancellationToken;

/// The [`ValidatorNodeService`] trait defines the common interface for running a validator node
/// service in the rollup node. The validator node listens to two sources of information to sync the
/// L2 chain:
///
/// 1. The data availability layer, with a watcher that listens for new updates. L2 inputs (L2
///    transaction batches + deposits) are then derived from the DA layer.
/// 2. The L2 sequencer, which produces unsafe L2 blocks and sends them to the network over p2p
///    gossip.
///
/// From these two sources, the validator node imports `unsafe` blocks from the L2 sequencer and
/// `safe` blocks from the L2 derivation pipeline into the L2 execution layer via the Engine API.
///
/// Finally, a state actor listens for new L2 block import events and updates the L2 state
/// accordingly, sending notifications to the other actors for synchronization.
///
/// ## Actor Communication
///
/// ```not_rust
/// ┌────────────┐
/// │L2 Sequencer│
/// │            ├───┐
/// │   Gossip   │   │   ┌────────────┐   ┌────────────┐   ┌────────────┐
/// └────────────┘   │   │            │   │            │   │            │
///                  ├──►│ Derivation │──►│ Engine API │──►│   State    │
/// ┌────────────┐   │   │            │   │            │   │            │
/// │     DA     │   │   └────────────┘   └┬───────────┘   └┬───────────┘
/// │            ├───┘              ▲      │                │
/// │   Watcher  │                  └──────┴────────────────┘
/// └────────────┘
/// ```
///
/// ## Types
///
/// - `DataAvailabilityWatcher`: The type of [`NodeActor`] to use for the DA watcher service.
/// - `DerivationPipeline`: The type of [Pipeline] to use for the service. Can be swapped out from
///   the default implementation for the sake of plugins like Alt DA.
/// - `Error`: The type of error for the service's entrypoint.
#[async_trait]
pub trait ValidatorNodeService {
    /// The type of [`NodeActor`] to use for the DA watcher service.
    type DataAvailabilityWatcher: NodeActor<Error: Display> + Send + Sync + 'static;
    /// The type of derivation pipeline to use for the service.
    type DerivationPipeline: Pipeline + SignalReceiver + Send + Sync + 'static;
    /// The type of error for the service's entrypoint.
    type Error: From<RpcLauncherError>
        + From<EngineStateBuilderError>
        + From<jsonrpsee::server::RegisterMethodError>
        + std::fmt::Debug;

    /// Returns a reference to the rollup node's [`RollupConfig`].
    fn config(&self) -> &RollupConfig;

    /// Creates a new [`NodeActor`] instance that watches the data availability layer. The
    /// `new_data_tx` channel is used to send updates on the data availability layer to the
    /// derivation pipeline. The `cancellation` token is used to gracefully shut down the actor.
    fn new_da_watcher(
        &self,
        new_data_tx: UnboundedSender<BlockInfo>,
        block_signer_tx: UnboundedSender<Address>,
        cancellation: CancellationToken,
        l1_watcher_inbound_queries: Option<tokio::sync::mpsc::Receiver<L1WatcherQueries>>,
    ) -> Self::DataAvailabilityWatcher;

    /// Creates a new instance of the [`Pipeline`] and initializes it. Returns the starting L2
    /// forkchoice state and the initialized derivation pipeline.
    async fn init_derivation(
        &self,
    ) -> Result<(L2ForkchoiceState, Self::DerivationPipeline), Self::Error>;

    /// Creates a new instance of the [`Network`].
    async fn init_network(&self) -> Result<Option<(Network, NetworkRpc)>, Self::Error>;

    /// Returns the [`RuntimeLauncher`] for the node.
    fn runtime(&self) -> RuntimeLauncher;

    /// Returns the [`EngineLauncher`]
    fn engine(&self) -> EngineLauncher;

    /// Returns the [`RpcLauncher`] for the node.
    fn rpc(&self) -> RpcLauncher;

    /// Starts the rollup node service.
    async fn start(&self) -> Result<(), Self::Error> {
        // Create a global cancellation token for graceful shutdown of tasks.
        let cancellation = CancellationToken::new();

        // Create channels for communication between actors.
        let (new_head_tx, new_head_rx) = mpsc::unbounded_channel();
        let (derived_payload_tx, derived_payload_rx) = mpsc::unbounded_channel();
        let (unsafe_block_tx, unsafe_block_rx) = mpsc::unbounded_channel();
        let (sync_complete_tx, sync_complete_rx) = mpsc::unbounded_channel();
        let (runtime_config_tx, runtime_config_rx) = mpsc::unbounded_channel();
        let (derivation_signal_tx, derivation_signal_rx) = mpsc::unbounded_channel();

        let (block_signer_tx, block_signer_rx) = mpsc::unbounded_channel();
        let (l1_watcher_queries_sender, l1_watcher_queries_recv) = tokio::sync::mpsc::channel(1024);
        let da_watcher = Some(self.new_da_watcher(
            new_head_tx,
            block_signer_tx,
            cancellation.clone(),
            Some(l1_watcher_queries_recv),
        ));

        let (_, derivation_pipeline) = self.init_derivation().await?;
        let (engine_l2_safe_tx, engine_l2_safe_rx) =
            tokio::sync::watch::channel(L2BlockInfo::default());
        let derivation = DerivationActor::new(
            derivation_pipeline,
            engine_l2_safe_rx,
            sync_complete_rx,
            derivation_signal_rx,
            derived_payload_tx,
            new_head_rx,
            cancellation.clone(),
        );
        let derivation = Some(derivation);

        /// The size of the RPC channel buffer for the engine actor.
        const ENGINE_RPC_CHANNEL_SIZE: usize = 1024;

        let (engine_query_sender, engine_query_recv) =
            tokio::sync::mpsc::channel(ENGINE_RPC_CHANNEL_SIZE);

        let runtime = self
            .runtime()
            .with_tx(runtime_config_tx)
            .with_cancellation(cancellation.clone())
            .launch();

        let launcher = self.engine();
        let client = launcher.client();
        let engine = launcher.launch().await?;
        let engine = EngineActor::new(
            std::sync::Arc::new(self.config().clone()),
            client,
            engine,
            engine_l2_safe_tx,
            sync_complete_tx,
            derivation_signal_tx,
            runtime_config_rx,
            derived_payload_rx,
            unsafe_block_rx,
            Some(engine_query_recv),
            cancellation.clone(),
        );
        let engine = Some(engine);

        let mut p2p_module = None;
        let network = (self.init_network().await?).map_or_else(
            || None,
            |(driver, module)| {
                p2p_module = Some(module);
                Some(NetworkActor::new(
                    driver,
                    unsafe_block_tx,
                    block_signer_rx,
                    cancellation.clone(),
                ))
            },
        );

        // The RPC Server should go last to let other actors register their rpc modules.
        let mut launcher = self.rpc();
        launcher = launcher.merge(p2p_module.map(|r| r.into_rpc())).map_err(Self::Error::from)?;
        let rollup_rpc = RollupRpc::new(engine_query_sender, l1_watcher_queries_sender);
        launcher = launcher.merge(Some(rollup_rpc.into_rpc())).map_err(Self::Error::from)?;
        let handle = launcher.launch().await?;
        let rpc = handle.map(|h| RpcActor::new(h, cancellation.clone()));

        spawn_and_wait!(
            cancellation,
            actors = [da_watcher, runtime, rpc, derivation, engine, network]
        );
        Ok(())
    }
}
