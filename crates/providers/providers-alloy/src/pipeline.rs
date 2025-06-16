//! Contains an online derivation pipeline.

use crate::{AlloyChainProvider, AlloyL2ChainProvider, OnlineBeaconClient, OnlineBlobProvider};
use async_trait::async_trait;
use core::fmt::Debug;
use kona_derive::{
    AttributesQueueStage, DerivationPipeline, EthereumDataSource, L2ChainProvider, OriginProvider,
    Pipeline, PipelineBuilder, PipelineErrorKind, PipelineResult, ResetSignal, Signal,
    SignalReceiver, StatefulAttributesBuilder, StepResult,
};
use kona_genesis::{RollupConfig, SystemConfig};
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use std::sync::Arc;

/// An online derivation pipeline.
pub type OnlineDerivationPipeline = DerivationPipeline<
    AttributesQueueStage<
        OnlineDataProvider,
        AlloyChainProvider,
        AlloyL2ChainProvider,
        OnlineAttributesBuilder,
    >,
    AlloyL2ChainProvider,
>;

/// An RPC-backed Ethereum data source.
pub type OnlineDataProvider =
    EthereumDataSource<AlloyChainProvider, OnlineBlobProvider<OnlineBeaconClient>>;

/// An RPC-backed payload attributes builder for the `AttributesQueue` stage of the derivation
/// pipeline.
pub type OnlineAttributesBuilder =
    StatefulAttributesBuilder<AlloyChainProvider, AlloyL2ChainProvider>;

/// An oracle-backed derivation pipeline.
#[derive(Debug)]
pub struct OnlinePipeline {
    /// The internal derivation pipeline.
    pub pipeline: OnlineDerivationPipeline,
}

impl OnlinePipeline {
    /// Constructs a new oracle-backed derivation pipeline.
    pub async fn new(
        cfg: Arc<RollupConfig>,
        l2_safe_head: L2BlockInfo,
        l1_origin: BlockInfo,
        blob_provider: OnlineBlobProvider<OnlineBeaconClient>,
        chain_provider: AlloyChainProvider,
        mut l2_chain_provider: AlloyL2ChainProvider,
    ) -> PipelineResult<Self> {
        let Self { mut pipeline } = Self::new_uninitialized(
            cfg.clone(),
            blob_provider,
            chain_provider,
            l2_chain_provider.clone(),
        );

        // Reset the pipeline to populate the initial L1/L2 cursor and system configuration in L1
        // Traversal.
        pipeline
            .signal(
                ResetSignal {
                    l2_safe_head,
                    l1_origin,
                    system_config: l2_chain_provider
                        .system_config_by_number(l2_safe_head.block_info.number, cfg.clone())
                        .await
                        .ok(),
                }
                .signal(),
            )
            .await?;

        Ok(Self { pipeline })
    }

    /// Constructs a new oracle-backed derivation pipeline that has not been populated with an L2
    /// safe head, L1 origin, or System Config. Before using, a [`ResetSignal`] must be sent to
    /// instantiate the pipeline state.
    pub fn new_uninitialized(
        cfg: Arc<RollupConfig>,
        blob_provider: OnlineBlobProvider<OnlineBeaconClient>,
        chain_provider: AlloyChainProvider,
        l2_chain_provider: AlloyL2ChainProvider,
    ) -> Self {
        let attributes = StatefulAttributesBuilder::new(
            cfg.clone(),
            l2_chain_provider.clone(),
            chain_provider.clone(),
        );
        let dap = EthereumDataSource::new_from_parts(chain_provider.clone(), blob_provider, &cfg);

        let pipeline = PipelineBuilder::new()
            .rollup_config(cfg.clone())
            .dap_source(dap)
            .l2_chain_provider(l2_chain_provider.clone())
            .chain_provider(chain_provider)
            .builder(attributes)
            .origin(BlockInfo::default())
            .build();

        Self { pipeline }
    }
}

#[async_trait]
impl SignalReceiver for OnlinePipeline {
    /// Receives a signal from the driver.
    async fn signal(&mut self, signal: Signal) -> PipelineResult<()> {
        self.pipeline.signal(signal).await
    }
}

impl OriginProvider for OnlinePipeline {
    /// Returns the optional L1 [BlockInfo] origin.
    fn origin(&self) -> Option<BlockInfo> {
        self.pipeline.origin()
    }
}

impl Iterator for OnlinePipeline {
    type Item = OpAttributesWithParent;

    fn next(&mut self) -> Option<Self::Item> {
        self.pipeline.next()
    }
}

#[async_trait]
impl Pipeline for OnlinePipeline {
    /// Peeks at the next [OpAttributesWithParent] from the pipeline.
    fn peek(&self) -> Option<&OpAttributesWithParent> {
        self.pipeline.peek()
    }

    /// Attempts to progress the pipeline.
    async fn step(&mut self, cursor: L2BlockInfo) -> StepResult {
        self.pipeline.step(cursor).await
    }

    /// Returns the rollup config.
    fn rollup_config(&self) -> &RollupConfig {
        self.pipeline.rollup_config()
    }

    /// Returns the [SystemConfig] by L2 number.
    async fn system_config_by_number(
        &mut self,
        number: u64,
    ) -> Result<SystemConfig, PipelineErrorKind> {
        self.pipeline.system_config_by_number(number).await
    }
}
