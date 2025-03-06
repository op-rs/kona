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
use kona_derive::traits::{ChainProvider, L2ChainProvider};
use kona_genesis::RollupConfig;
use kona_protocol::BatchValidationProvider;
use kona_providers_alloy::{
    AlloyChainProvider, AlloyChainProviderError, AlloyL2ChainProvider, OnlineBeaconClient,
    OnlineBlobProvider, OnlinePipeline,
};
use op_alloy_network::Optimism;
use std::sync::Arc;
use thiserror::Error;
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

mod sync_start;
pub use sync_start::{L2ForkchoiceState, SyncStartError, find_starting_forkchoice};

const PROVIDER_CACHE_SIZE: usize = 1024;

/// Spawns a set of parallel actors in a [JoinSet], and cancels all actors if any of them fail. The
/// type of the error in the [NodeActor]s is erased to avoid having to specify a common error type
/// between actors.
///
/// [JoinSet]: tokio::task::JoinSet
macro_rules! spawn_and_wait {
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
    pub async fn start(self) -> Result<(), RollupNodeError> {
        info!(target: "rollup_node", "Starting node services...");

        // Create a global broadcast channel for communication between producers and actors, as well
        // as a cancellation token for graceful shutdown.
        let (sender, receiver) = broadcast::channel::<NodeEvent>(1024);
        let cancellation = CancellationToken::new();

        // Create the caching L1/L2 EL providers for derivation.
        let mut l1_derivation_provider =
            AlloyChainProvider::new(self.l1_provider.clone(), PROVIDER_CACHE_SIZE);
        let mut l2_derivation_provider = AlloyL2ChainProvider::new(
            self.l2_provider.clone(),
            self.config.clone(),
            PROVIDER_CACHE_SIZE,
        );

        // Find the starting forkchoice state.
        let starting_forkchoice = find_starting_forkchoice(
            self.config.as_ref(),
            &mut l1_derivation_provider,
            &mut l2_derivation_provider,
        )
        .await?;

        // Create the actors.
        let l1_watcher_actor =
            L1WatcherRpc::new(self.l1_provider.clone(), sender.clone(), cancellation.clone());
        let derivation_actor = {
            let starting_origin_num = starting_forkchoice.safe.l1_origin.number -
                self.config.channel_timeout(starting_forkchoice.safe.block_info.timestamp);
            let starting_origin =
                l1_derivation_provider.block_info_by_number(starting_origin_num).await?;

            let pipeline = OnlinePipeline::new(
                self.config.clone(),
                starting_forkchoice.safe,
                starting_origin,
                OnlineBlobProvider::init(self.l1_beacon).await,
                l1_derivation_provider,
                l2_derivation_provider,
            )
            .await;

            match pipeline {
                Ok(p) => DerivationActor::new(
                    p,
                    starting_forkchoice.safe,
                    sender,
                    receiver,
                    cancellation.clone(),
                ),
                Err(e) => {
                    tracing::error!(target: "rollup_node", "Failed to initialize derivation pipeline: {e}");
                    return Ok(());
                }
            }
        };

        spawn_and_wait!(cancellation, actors = [l1_watcher_actor, derivation_actor]);

        Ok(())
    }
}

/// Errors that can occur during the operation of the [RollupNode].
#[derive(Error, Debug)]
pub enum RollupNodeError {
    /// An error occurred while finding the sync starting point.
    #[error(transparent)]
    SyncStart(#[from] SyncStartError),
    /// An error occurred while initializing the derivation pipeline.
    #[error(transparent)]
    AlloyChainProvider(#[from] AlloyChainProviderError),
}
