//! The core [`RollupNodeService`] trait
use crate::{
    AttributesBuilderConfig, DerivationContext, EngineContext, L1WatcherRpcContext, NetworkContext,
    NodeActor, NodeMode, RpcContext, SequencerContext, SequencerInboundData,
    actors::{
        DerivationInboundChannels, EngineInboundData, L1WatcherRpcInboundChannels,
        NetworkInboundData, PipelineBuilder,
    },
    service::spawn_and_wait,
};
use async_trait::async_trait;
use kona_derive::{AttributesBuilder, Pipeline, SignalReceiver};
use std::fmt::Display;
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
/// In sequencer mode, the node is responsible for producing unsafe L2 blocks and sending them to
/// the network over p2p gossip. The node also listens for L1 finalized block updates and finalizes
/// `safe` blocks that it has derived when L1 finalized block updates are received.
///
/// ## Types
///
/// - `DataAvailabilityWatcher`: The type of [`NodeActor`] to use for the DA watcher service.
/// - `DerivationPipeline`: The type of [Pipeline] to use for the service. Can be swapped out from
///   the default implementation for the sake of plugins like Alt DA.
/// - `Error`: The type of error for the service's entrypoint.
#[async_trait]
pub trait RollupNodeService {
    /// The type of [`NodeActor`] to use for the DA watcher service.
    type DataAvailabilityWatcher: NodeActor<
            Error: Display,
            OutboundData = L1WatcherRpcContext,
            InboundData = L1WatcherRpcInboundChannels,
        >;

    /// The type of derivation pipeline to use for the service.
    type DerivationPipeline: Pipeline + SignalReceiver + Send + Sync + 'static;

    /// The type of derivation actor to use for the service.
    type DerivationActor: NodeActor<
            Error: Display,
            Builder: PipelineBuilder<Pipeline = Self::DerivationPipeline>,
            OutboundData = DerivationContext,
            InboundData = DerivationInboundChannels,
        >;

    /// The type of engine actor to use for the service.
    type EngineActor: NodeActor<Error: Display, OutboundData = EngineContext, InboundData = EngineInboundData>;

    /// The type of network actor to use for the service.
    type NetworkActor: NodeActor<Error: Display, OutboundData = NetworkContext, InboundData = NetworkInboundData>;

    /// The type of attributes builder to use for the sequener.
    type AttributesBuilder: AttributesBuilder + Send + Sync + 'static;

    /// The type of sequencer actor to use for the service.
    type SequencerActor: NodeActor<
            Error: Display,
            OutboundData = SequencerContext,
            Builder: AttributesBuilderConfig<AB = Self::AttributesBuilder>,
            InboundData = SequencerInboundData,
        >;

    /// The type of rpc actor to use for the service.
    type RpcActor: NodeActor<Error: Display, OutboundData = RpcContext, InboundData = ()>;

    /// The mode of operation for the node.
    fn mode(&self) -> NodeMode;

    /// Returns a DA watcher builder for the node.
    fn da_watcher_builder(&self) -> <Self::DataAvailabilityWatcher as NodeActor>::Builder;

    /// Returns a derivation builder for the node.
    fn derivation_builder(&self) -> <Self::DerivationActor as NodeActor>::Builder;

    /// Creates a network builder for the node.
    fn network_builder(&self) -> <Self::NetworkActor as NodeActor>::Builder;

    /// Returns an engine builder for the node.
    fn engine_builder(&self) -> <Self::EngineActor as NodeActor>::Builder;

    /// Returns an rpc builder for the node.
    fn rpc_builder(&self) -> Option<<Self::RpcActor as NodeActor>::Builder>;

    /// Returns the sequencer builder for the node.
    fn sequencer_builder(&self) -> <Self::SequencerActor as NodeActor>::Builder;

    /// Starts the rollup node service.
    async fn start(&self) -> Result<(), String> {
        // Create a global cancellation token for graceful shutdown of tasks.
        let cancellation = CancellationToken::new();

        // Create the DA watcher actor.
        let (L1WatcherRpcInboundChannels { inbound_queries: da_watcher_rpc }, da_watcher) =
            Self::DataAvailabilityWatcher::build(self.da_watcher_builder());

        // Create the derivation actor.
        let (
            DerivationInboundChannels {
                derivation_signal_tx,
                l1_head_updates_tx,
                engine_l2_safe_head_tx,
                el_sync_complete_tx,
            },
            derivation,
        ) = Self::DerivationActor::build(self.derivation_builder());

        // Create the engine actor.
        let (
            EngineInboundData {
                build_request_tx,
                seal_request_tx,
                attributes_tx,
                unsafe_block_tx,
                reset_request_tx,
                inbound_queries_tx: engine_rpc,
                finalized_l1_block_tx,
            },
            engine,
        ) = Self::EngineActor::build(self.engine_builder());

        // Create the p2p actor.
        let (
            NetworkInboundData {
                signer,
                p2p_rpc: network_rpc,
                gossip_payload_tx,
                admin_rpc: net_admin_rpc,
            },
            network,
        ) = Self::NetworkActor::build(self.network_builder());

        // Create the RPC server actor.
        let (_, rpc) = self.rpc_builder().map(Self::RpcActor::build).unzip();

        let (sequencer_inbound_data, sequencer) = self
            .mode()
            .is_sequencer()
            .then_some(Self::SequencerActor::build(self.sequencer_builder()))
            .unzip();

        spawn_and_wait!(
            cancellation,
            actors = [
                rpc.map(|r| (
                    r,
                    RpcContext {
                        cancellation: cancellation.clone(),
                        p2p_network: network_rpc,
                        network_admin: net_admin_rpc,
                        sequencer_admin: sequencer_inbound_data.as_ref().map(|s| s.admin_query_tx.clone()),
                        l1_watcher_queries: da_watcher_rpc,
                        engine_query: engine_rpc,
                    }
                )),
                sequencer.map(|s| (
                    s,
                    SequencerContext {
                        l1_head_rx: l1_head_updates_tx.subscribe(),
                        reset_request_tx: reset_request_tx.clone(),
                        build_request_tx: build_request_tx.expect(
                            "`build_request_tx` not set while in sequencer mode. This should never happen.",
                        ),
                        seal_request_tx: seal_request_tx.expect(
                            "`seal_request_tx` not set while in sequencer mode. This should never happen.",
                        ),
                        gossip_payload_tx,
                        cancellation: cancellation.clone(),
                    })
                ),
                Some((
                    network,
                    NetworkContext { blocks: unsafe_block_tx, cancellation: cancellation.clone() }
                )),
                Some((
                    da_watcher,
                    L1WatcherRpcContext {
                        latest_head: l1_head_updates_tx,
                        latest_finalized: finalized_l1_block_tx,
                        block_signer_sender: signer,
                        cancellation: cancellation.clone(),
                    })
                ),
                Some((
                    derivation,
                    DerivationContext {
                        reset_request_tx: reset_request_tx.clone(),
                        derived_attributes_tx: attributes_tx,
                        cancellation: cancellation.clone(),
                })),
                Some((engine,
                    EngineContext {
                        engine_l2_safe_head_tx,
                        engine_unsafe_head_tx: sequencer_inbound_data
                            .map(|s| s.unsafe_head_tx),
                        sync_complete_tx: el_sync_complete_tx,
                        derivation_signal_tx,
                        cancellation: cancellation.clone(),
                    })
                ),
            ]
        );
        Ok(())
    }
}
