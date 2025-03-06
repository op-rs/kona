//! Core module for the [RollupNode] and child tasks.
//!
//! The architecture of the rollup node is event-driven, with [NodeActor]s to drive syncing the L2
//! chain. The [RollupNode] orchestrates all of these components.
//!
//! ```text
//! ┌────────────┐
//! │L2 Sequencer│
//! │            ├───┐
//! │   Gossip   │   │   ┌────────────┐   ┌────────────┐   ┌────────────┐
//! └────────────┘   │   │            │   │            │   │            │
//!                  ├──►│ Derivation │──►│ Engine API │──►│   State    │
//! ┌────────────┐   │   │            │   │            │   │            │
//! │  L1 Chain  │   │   └────────────┘   └┬───────────┘   └┬───────────┘
//! │            ├───┘              ▲      │                │
//! │  Watcher   │                  └──────┴────────────────┘
//! └────────────┘
//! ```

#![allow(unused)]

use alloy_provider::RootProvider;
use kona_genesis::RollupConfig;
use kona_providers_alloy::{
    AlloyChainProvider, AlloyL2ChainProvider, OnlineBeaconClient, OnlineBlobProvider,
    OnlinePipeline,
};
use op_alloy_network::Optimism;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::info;

mod actors;
pub use actors::{DerivationActor, L1WatcherRpc};

mod builder;
pub use builder::RollupNodeBuilder;

mod traits;
pub use traits::NodeActor;

mod events;
pub use events::NodeEvent;

const PROVIDER_CACHE_SIZE: usize = 1024;

/// Spawns a set of parallel actors in a [JoinSet], and cancels all actors if any of them fail. The
/// type of the error in the [NodeActor]s is erased to avoid having to specify a common error type
/// between actors.
///
/// [JoinSet]: tokio::task::JoinSet
macro_rules! spawn_actors {
    ($cancellation:expr, actors = [$($actor:expr$(,)?)*]) => {
        let mut task_handles = tokio::task::JoinSet::new();

        $(
            task_handles.spawn(async move {
                if let Err(e) = $actor.start().await {
                    tracing::error!(target: "rollup_node", "{e}");
                }
            });
        )*

        while let Some(result) = task_handles.join_next().await {
            if let Err(e) = result {
                tracing::error!(target: "rollup_node", "Error joining subroutine: {e}");

                // Cancel all tasks and gracefully shutdown.
                $cancellation.cancel();
            }
        }
    };
}

/// The [RollupNode] service orchestrates the various sub-components of the rollup node. It itself
/// is a [NodeActor], and it receives events emitted by other actors in the system in order to track
/// state updates.
#[allow(unused)]
pub struct RollupNode {
    /// The rollup configuration.
    config: Arc<RollupConfig>,
    /// The L1 EL provider.
    l1_provider: RootProvider,
    /// The L1 beacon API.
    l1_beacon: OnlineBeaconClient,
    /// The L2 EL provider.
    l2_provider: RootProvider<Optimism>,
    /// The L2 engine.
    ///
    /// TODO: Place L2 Engine API client here once it's ready.
    l2_engine: (),
}

impl RollupNode {
    /// Creates a new [RollupNodeBuilder] to configure the node.
    pub fn builder(config: RollupConfig) -> RollupNodeBuilder {
        RollupNodeBuilder::new(config)
    }

    /// Starts the node.
    ///
    /// TODO: Ensure external shutdown signals are intercepted and respect the graceful cancellation
    /// of subroutines, to ensure that the node can be stopped cleanly.
    pub async fn start(self) {
        info!(target: "rollup_node", "Starting node services...");

        // Create a global broadcast channel for communication between producers and actors, as well
        // as a cancellation token for graceful shutdown.
        let (s, r) = broadcast::channel::<NodeEvent>(1024);
        let cancellation = CancellationToken::new();

        // Create the actors.
        let l1_watcher_rpc =
            L1WatcherRpc::new(self.l1_provider.clone(), s.clone(), cancellation.clone());
        let derivation_actor = {
            let pipeline = OnlinePipeline::new(
                self.config.clone(),
                todo!(), // Need sync start
                todo!(), // Need sync start
                OnlineBlobProvider::init(self.l1_beacon).await,
                AlloyChainProvider::new(self.l1_provider, PROVIDER_CACHE_SIZE),
                AlloyL2ChainProvider::new(
                    self.l2_provider.clone(),
                    self.config.clone(),
                    PROVIDER_CACHE_SIZE,
                ),
            )
            .await;

            match pipeline {
                Ok(p) => DerivationActor::new(p, todo!(), s, r, cancellation.clone()),
                Err(e) => {
                    tracing::error!(target: "rollup_node", "Failed to initialize derivation pipeline: {e}");
                    return;
                }
            }
        };

        spawn_actors!(cancellation, actors = [l1_watcher_rpc, derivation_actor]);
    }
}
