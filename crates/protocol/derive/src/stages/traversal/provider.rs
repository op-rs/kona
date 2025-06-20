//! L1TraversalProvider is a provider for the L1Traversal stage.
//! It is used to retrieve the next L1 block and the origin block.
//! It is also used to advance the origin block.
//! It is also used to signal the L1Traversal stage.

use crate::{
    ChainProvider, L1RetrievalProvider, L1Traversal, ManagedTraversal, OriginAdvancer,
    OriginProvider, PipelineResult, Signal, SignalReceiver,
};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::BlockInfo;
use std::sync::Arc;

/// `TraversalProvider` is a mux between the autonomous `L1Traversal` stage and the
/// externally-controlled `ManagedTraversal` stage.
///
/// - Before interop/managed mode, it delegates to `L1Traversal` for autonomous L1 chain traversal.
/// - After switching to managed mode, it delegates to `ManagedTraversal`, which only advances when
///   instructed externally.
///
/// This allows the pipeline to seamlessly switch between normal and managed operation.
#[derive(Debug)]
pub struct TraversalProvider<P: ChainProvider + Send + Sync + 'static> {
    /// The autonomous L1 traversal stage (used before interop/managed mode).
    pub l1_traversal: L1Traversal<P>,
    /// The managed L1 traversal stage (used after interop/managed mode).
    pub managed_traversal: ManagedTraversal<P>,
    /// If true, use managed traversal; otherwise, use autonomous traversal.
    pub is_managed: bool,
    /// A reference to the rollup config.
    pub rollup_config: Arc<RollupConfig>,
}

impl<P: ChainProvider + Send + Sync + 'static> TraversalProvider<P> {
    /// Create a new mux provider from the given traversal stages.
    pub const fn new(
        l1_traversal: L1Traversal<P>,
        managed_traversal: ManagedTraversal<P>,
        rollup_config: Arc<RollupConfig>,
    ) -> Self {
        Self { l1_traversal, managed_traversal, is_managed: false, rollup_config }
    }

    /// Update the is_managed flag based on the current origin's timestamp and the rollup config's
    /// interop activation.
    fn update_mode_from_origin(&mut self) {
        // Use the origin from either traversal (should be the same in practice)
        let origin = self.l1_traversal.origin().or_else(|| self.managed_traversal.origin());
        if let Some(block) = origin {
            self.is_managed = self.rollup_config.is_interop_active(block.timestamp);
        }
    }
}

#[async_trait]
impl<P: ChainProvider + Send + Sync + 'static> L1RetrievalProvider for TraversalProvider<P> {
    /// Retrieve the next L1 block from the active traversal stage.
    async fn next_l1_block(&mut self) -> PipelineResult<Option<BlockInfo>> {
        self.update_mode_from_origin();
        if self.is_managed {
            self.managed_traversal.next_l1_block().await
        } else {
            self.l1_traversal.next_l1_block().await
        }
    }

    /// Get the batcher address from the active traversal stage.
    fn batcher_addr(&self) -> alloy_primitives::Address {
        if self.is_managed {
            self.managed_traversal.batcher_addr()
        } else {
            self.l1_traversal.batcher_addr()
        }
    }
}

impl<P: ChainProvider + Send + Sync + 'static> OriginProvider for TraversalProvider<P> {
    /// Get the current L1 origin block from the active traversal stage.
    fn origin(&self) -> Option<BlockInfo> {
        if self.is_managed { self.managed_traversal.origin() } else { self.l1_traversal.origin() }
    }
}

#[async_trait]
impl<P: ChainProvider + Send + Sync + 'static> OriginAdvancer for TraversalProvider<P> {
    /// Advance the L1 origin in the active traversal stage.
    async fn advance_origin(&mut self) -> PipelineResult<()> {
        self.update_mode_from_origin();
        if self.is_managed {
            self.managed_traversal.advance_origin().await
        } else {
            self.l1_traversal.advance_origin().await
        }
    }
}

#[async_trait]
impl<P: ChainProvider + Send + Sync + 'static> SignalReceiver for TraversalProvider<P> {
    /// Pass a signal to the active traversal stage.
    async fn signal(&mut self, signal: Signal) -> PipelineResult<()> {
        self.update_mode_from_origin();
        if self.is_managed {
            self.managed_traversal.signal(signal).await
        } else {
            self.l1_traversal.signal(signal).await
        }
    }
}
