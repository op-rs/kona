//! Contains the [`ManagedTraversal`] stage of the derivation pipeline.

use crate::{
    ActivationSignal, ChainProvider, L1RetrievalProvider, OriginAdvancer, OriginProvider,
    PipelineError, PipelineResult, ResetError, ResetSignal, Signal, SignalReceiver,
};
use alloc::{boxed::Box, sync::Arc};
use alloy_primitives::Address;
use async_trait::async_trait;
use kona_genesis::{RollupConfig, SystemConfig};
use kona_protocol::BlockInfo;

/// The [`ManagedTraversal`] stage of the derivation pipeline.
///
/// This stage sits at the bottom of the pipeline, holding a handle to the data source
/// (a [`ChainProvider`] implementation) and the current L1 [`BlockInfo`] in the pipeline,
/// which are used to traverse the L1 chain. When the [`ManagedTraversal`] stage is advanced,
/// it fetches the next L1 [`BlockInfo`] from the data source and updates the [`SystemConfig`]
/// with the receipts from the block.
#[derive(Debug, Clone)]
pub struct ManagedTraversal<Provider: ChainProvider> {
    /// The current block in the traversal stage.
    pub block: Option<BlockInfo>,
    /// The data source for the traversal stage.
    pub data_source: Provider,
    /// Indicates whether the block has been consumed by other stages.
    pub done: bool,
    /// The system config.
    pub system_config: SystemConfig,
    /// A reference to the rollup config.
    pub rollup_config: Arc<RollupConfig>,
}

#[async_trait]
impl<F: ChainProvider + Send> L1RetrievalProvider for ManagedTraversal<F> {
    fn batcher_addr(&self) -> Address {
        self.system_config.batcher_address
    }

    async fn next_l1_block(&mut self) -> PipelineResult<Option<BlockInfo>> {
        if !self.done {
            self.done = true;
            Ok(self.block)
        } else {
            Err(PipelineError::Eof.temp())
        }
    }
}

impl<F: ChainProvider> ManagedTraversal<F> {
    /// Creates a new [`ManagedTraversal`] instance.
    pub fn new(data_source: F, cfg: Arc<RollupConfig>) -> Self {
        Self {
            block: Some(BlockInfo::default()),
            data_source,
            done: false,
            system_config: SystemConfig::default(),
            rollup_config: cfg,
        }
    }

    /// Update the origin block in the traversal stage.
    fn update_origin(&mut self, block: BlockInfo) {
        self.done = false;
        self.block = Some(block);
        kona_macros::set!(gauge, crate::metrics::Metrics::PIPELINE_ORIGIN, block.number as f64);
    }

    /// Provide the next block to the traversal stage.
    async fn provide_next_block(&mut self, block_info: BlockInfo) -> PipelineResult<()> {
        if !self.done {
            debug!(target: "traversal", "Not finished consuming block, ignoring provided block.");
            return Ok(());
        }
        let Some(block) = self.block else {
            return Err(PipelineError::MissingOrigin.temp());
        };
        if block.number + 1 != block_info.number {
            // Safe to ignore.
            // The next step will exhaust l1 and get the correct next l1 block.
            return Ok(());
        }
        if block.hash != block_info.parent_hash {
            return Err(
                ResetError::NextL1BlockHashMismatch(block.hash, block_info.parent_hash).reset()
            );
        }

        // Fetch receipts for the next l1 block and update the system config.
        let receipts =
            self.data_source.receipts_by_hash(block_info.hash).await.map_err(Into::into)?;

        let addr = self.rollup_config.l1_system_config_address;
        let active = self.rollup_config.is_ecotone_active(block_info.timestamp);
        match self.system_config.update_with_receipts(&receipts[..], addr, active) {
            Ok(true) => {
                let next = block_info.number as f64;
                kona_macros::set!(gauge, crate::Metrics::PIPELINE_LATEST_SYS_CONFIG_UPDATE, next);
                info!(target: "traversal", "System config updated at block {next}.");
            }
            Ok(false) => { /* Ignore, no update applied */ }
            Err(err) => {
                error!(target: "traversal", ?err, "Failed to update system config at block {}", block_info.number);
                kona_macros::set!(
                    gauge,
                    crate::Metrics::PIPELINE_SYS_CONFIG_UPDATE_ERROR,
                    block_info.number as f64
                );
                return Err(PipelineError::SystemConfigUpdate(err).crit());
            }
        }

        // Update the origin block.
        self.update_origin(block_info);

        Ok(())
    }
}

#[async_trait]
impl<F: ChainProvider + Send> OriginAdvancer for ManagedTraversal<F> {
    async fn advance_origin(&mut self) -> PipelineResult<()> {
        if !self.done {
            debug!(target: "traversal", "Not finished consuming block, ignoring advance call.");
            return Ok(());
        }
        return Err(PipelineError::Eof.temp());
    }
}

impl<F: ChainProvider> OriginProvider for ManagedTraversal<F> {
    fn origin(&self) -> Option<BlockInfo> {
        self.block
    }
}

#[async_trait]
impl<F: ChainProvider + Send> SignalReceiver for ManagedTraversal<F> {
    async fn signal(&mut self, signal: Signal) -> PipelineResult<()> {
        match signal {
            Signal::Reset(ResetSignal { l1_origin, system_config, .. }) |
            Signal::Activation(ActivationSignal { l1_origin, system_config, .. }) => {
                self.update_origin(l1_origin);
                self.system_config = system_config.expect("System config must be provided.");
            }
            Signal::ProvideBlock(block_info) => self.provide_next_block(block_info).await?,
            _ => {}
        }

        Ok(())
    }
}
