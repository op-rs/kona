//! Core module for the [RollupNode] and child tasks.
//!
//! The architecture of the rollup node is event-driven, using [NodeProducer]s to watch external
//! data sources and emit events for [NodeActor]s to consume and process. The [RollupNode]
//! orchestrates all of these components, and is itself a [NodeActor] that processes incoming events
//! to update relevant state.

use alloy_provider::RootProvider;
use tokio::{sync::broadcast, task::JoinSet};
use tokio_util::sync::CancellationToken;

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
pub struct RollupNode {
    /// The L1 EL provider.
    l1_provider: RootProvider,
}

impl RollupNode {
    /// Creates a new [RollupNodeBuilder] to configure the node.
    pub fn builder() -> RollupNodeBuilder {
        RollupNodeBuilder::default()
    }

    /// Starts the node.
    pub async fn start(self) {
        // Create a global broadcast channel for communication between producers and actors, as well
        // as a cancellation token for graceful shutdown.
        let (s, _) = broadcast::channel::<NodeEvent>(1024);
        let cancellation = CancellationToken::new();

        // Create a set to manage all spawned tasks.
        let mut task_handles = JoinSet::new();

        // Create the producers.
        let l1_watcher_rpc =
            L1WatcherRpc::new(self.l1_provider.clone(), s.clone(), cancellation.clone());

        // Spawn the producers.
        task_handles.spawn(l1_watcher_rpc.start());

        // Wait for all tasks to complete.
        while let Some(result) = task_handles.join_next().await {
            if let Err(e) = result {
                error!(target: "rollup_node", "{e}");
            }
        }
    }
}
