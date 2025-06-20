//! L1TraversalProvider is a provider for the L1Traversal stage.
//! It is used to retrieve the next L1 block and the origin block.
//! It is also used to advance the origin block.
//! It is also used to signal the L1Traversal stage.

use crate::{
    ChainProvider, L1RetrievalProvider, L1Traversal, ManagedTraversal, OriginAdvancer,
    OriginProvider, PipelineResult, Signal, SignalReceiver,
};
use alloc::sync::Arc;
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::BlockInfo;

/// `TraversalProvider` is a mux between the autonomous `L1Traversal` stage and the
/// externally-controlled `ManagedTraversal` stage.
///
/// - Before interop/managed mode, it delegates to `L1Traversal` for autonomous L1 chain traversal.
/// - After switching to managed mode, it delegates to `ManagedTraversal`, which only advances when
///   instructed externally.
///
/// This allows the pipeline to seamlessly switch between normal and managed operation.
#[derive(Debug)]
pub struct TraversalProvider<P: ChainProvider + Send + Sync> {
    /// The autonomous L1 traversal stage (used before interop/managed mode).
    pub l1_traversal: L1Traversal<P>,
    /// The managed L1 traversal stage (used after interop/managed mode).
    pub managed_traversal: ManagedTraversal<P>,
    /// If true, use managed traversal; otherwise, use autonomous traversal.
    pub is_managed: bool,
    /// A reference to the rollup config.
    pub rollup_config: Arc<RollupConfig>,
}

impl<P: ChainProvider + Send + Sync> TraversalProvider<P> {
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
        if let Some(block) = self.origin() {
            self.is_managed = self.rollup_config.is_interop_active(block.timestamp);
        }
    }
}

#[async_trait]
impl<P: ChainProvider + Send + Sync> L1RetrievalProvider for TraversalProvider<P> {
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

impl<P: ChainProvider + Send + Sync> OriginProvider for TraversalProvider<P> {
    /// Get the current L1 origin block from the active traversal stage.
    fn origin(&self) -> Option<BlockInfo> {
        if self.is_managed { self.managed_traversal.origin() } else { self.l1_traversal.origin() }
    }
}

#[async_trait]
impl<P: ChainProvider + Send + Sync> OriginAdvancer for TraversalProvider<P> {
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
impl<P: ChainProvider + Send + Sync> SignalReceiver for TraversalProvider<P> {
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::TestChainProvider;

    fn make_block(number: u64, timestamp: u64) -> BlockInfo {
        BlockInfo { number, timestamp, ..BlockInfo::default() }
    }

    fn make_provider_with_origin(
        origin: BlockInfo,
        interop_time: Option<u64>,
    ) -> TraversalProvider<TestChainProvider> {
        let mut l1_provider = TestChainProvider::default();
        l1_provider.insert_block(origin.number, origin);
        let mut l1_traversal =
            L1Traversal::new(l1_provider.clone(), Arc::new(RollupConfig::default()));
        let mut managed_traversal =
            ManagedTraversal::new(l1_provider, Arc::new(RollupConfig::default()));
        l1_traversal.block = Some(origin);
        managed_traversal.block = Some(origin);
        let mut cfg = RollupConfig::default();
        cfg.hardforks.interop_time = interop_time;
        TraversalProvider::new(l1_traversal, managed_traversal, Arc::new(cfg))
    }

    #[test]
    fn test_autonomous_mode_before_interop() {
        let origin = make_block(1, 100);
        let mut provider = make_provider_with_origin(origin, Some(200));
        provider.update_mode_from_origin();
        assert!(!provider.is_managed, "Should be in autonomous mode before interop");
        assert_eq!(provider.origin(), Some(origin));
    }

    #[test]
    fn test_managed_mode_after_interop() {
        let origin = make_block(1, 300);
        let mut provider = make_provider_with_origin(origin, Some(200));
        provider.update_mode_from_origin();
        assert!(provider.is_managed, "Should be in managed mode after interop");
        assert_eq!(provider.origin(), Some(origin));
    }

    #[test]
    fn test_transition_to_managed_mode() {
        let origin = make_block(1, 199);
        let mut provider = make_provider_with_origin(origin, Some(200));
        provider.update_mode_from_origin();
        assert!(!provider.is_managed, "Should start in autonomous mode");
        // Simulate advancing to interop
        let new_origin = make_block(2, 200);
        provider.l1_traversal.block = Some(new_origin);
        provider.update_mode_from_origin();
        assert!(provider.is_managed, "Should transition to managed mode at interop");
    }

    #[tokio::test]
    async fn test_transition_back_to_autonomous_mode() {
        let origin = make_block(1, 300);
        let mut provider = make_provider_with_origin(origin, Some(200));
        provider.update_mode_from_origin();
        assert!(provider.is_managed, "Should start in managed mode");
        // Simulate reorg to before interop
        let new_origin = make_block(2, 100);
        provider.l1_traversal.block = Some(new_origin);
        // Send a reset signal to update managed_traversal's origin as well
        provider
            .signal(Signal::Reset(crate::types::ResetSignal {
                l1_origin: new_origin,
                system_config: Some(kona_genesis::SystemConfig::default()),
                ..Default::default()
            }))
            .await
            .unwrap();
        provider.update_mode_from_origin();
        assert!(!provider.is_managed, "Should transition back to autonomous mode before interop");
    }
}
