//! Core module for the [RollupNode] and child tasks.
//!
//! The architecture of the rollup node is event-driven, using [NodeProducer]s to watch external
//! data sources and emit events for [NodeActor]s to consume and process. The [RollupNode]
//! orchestrates all of these components, and is itself a [NodeActor] that processes incoming events
//! to update relevant state.

use alloy_provider::RootProvider;
use kona_genesis::RollupConfig;
use kona_providers_alloy::OnlineBeaconClient;
use op_alloy_network::Optimism;
use tokio::{sync::broadcast, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

mod producers;
pub use producers::L1WatcherRpc;

mod actors;

mod builder;
pub use builder::RollupNodeBuilder;

mod traits;
pub use traits::{NodeActor, NodeProducer};

mod events;
pub use events::NodeEvent;

/// The [RollupNode] service orchestrates the various sub-components of the rollup node. It itself
/// is a [NodeActor], and it receives events emitted by other actors in the system in order to track
/// state updates.
#[allow(unused)]
pub struct RollupNode {
    /// The rollup configuration.
    config: RollupConfig,
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
        info!(target: "rollup_node", "Starting node...");

        // Create a global broadcast channel for communication between producers and actors, as well
        // as a cancellation token for graceful shutdown.
        let (s, _) = broadcast::channel::<NodeEvent>(1024);
        let cancellation = CancellationToken::new();

        // Create a set to manage all spawned tasks.
        let mut task_handles = JoinSet::new();

        // Create the producers.
        let l1_watcher_rpc =
            L1WatcherRpc::new(self.l1_provider.clone(), s.clone(), cancellation.clone());
        // TODO: Sequencer block gossip producer, if not running in sequencer mode.

        // Create the actors.
        // TODO: Derivation actor.
        // TODO: Engine controller actor.
        // TODO: Sequencer actor, if running in sequencer mode.

        // Spawn the producers.
        task_handles.spawn(l1_watcher_rpc.start());

        // Wait for all tasks to complete.
        while let Some(result) = task_handles.join_next().await {
            if let Err(e) = result {
                error!(target: "rollup_node", "Error joining subroutine: {e}");

                // Cancel all tasks and gracefully shutdown.
                cancellation.cancel();
            }
        }
    }
}
