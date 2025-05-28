//! Contains the forkchoice state for the L2.

use crate::SyncStartError;
use alloy_eips::BlockNumberOrTag;
use alloy_provider::{Provider, RootProvider};
use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;
use op_alloy_network::Optimism;
use std::fmt::Display;

/// An unsafe, safe, and finalized [L2BlockInfo] returned by the [crate::find_starting_forkchoice]
/// function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L2ForkchoiceState {
    /// The unsafe L2 block.
    pub un_safe: L2BlockInfo,
    /// The safe L2 block.
    pub safe: L2BlockInfo,
    /// The finalized L2 block.
    pub finalized: L2BlockInfo,
}

impl Display for L2ForkchoiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FINALIZED: {} (#{}) | SAFE: {} (#{}) | UNSAFE: {} (#{})",
            self.finalized.block_info.hash,
            self.finalized.block_info.number,
            self.safe.block_info.hash,
            self.safe.block_info.number,
            self.un_safe.block_info.hash,
            self.un_safe.block_info.number,
        )
    }
}

impl L2ForkchoiceState {
    /// Fetches the current forkchoice state of the L2 execution layer.
    ///
    /// - The finalized block may not always be available. If it is not, we fall back to genesis.
    /// - The safe block may not always be available. If it is not, we fall back to the finalized
    ///   block.
    /// - The unsafe block is always assumed to be available.
    pub async fn current(
        cfg: &RollupConfig,
        l2_provider: &RootProvider<Optimism>,
    ) -> Result<Self, SyncStartError> {
        let finalized =
            {
                let rpc_block =
                    match l2_provider.get_block(BlockNumberOrTag::Finalized.into()).full().await {
                        Ok(Some(block)) => block,
                        Ok(None) => l2_provider
                            .get_block(cfg.genesis.l2.number.into())
                            .full()
                            .await?
                            .ok_or(SyncStartError::BlockNotFound(cfg.genesis.l2.number.into()))?,
                        Err(e) => return Err(e.into()),
                    }
                    .into_consensus();

                L2BlockInfo::from_block_and_genesis(&rpc_block, &cfg.genesis)?
            };
        let safe = match l2_provider.get_block(BlockNumberOrTag::Safe.into()).full().await {
            Ok(Some(block)) => {
                L2BlockInfo::from_block_and_genesis(&block.into_consensus(), &cfg.genesis)?
            }
            Ok(None) => finalized,
            Err(e) => return Err(e.into()),
        };
        let un_safe = {
            let rpc_block = l2_provider
                .get_block(BlockNumberOrTag::Latest.into())
                .full()
                .await?
                .ok_or(SyncStartError::BlockNotFound(BlockNumberOrTag::Latest.into()))?;
            L2BlockInfo::from_block_and_genesis(&rpc_block.into_consensus(), &cfg.genesis)?
        };

        Ok(Self { un_safe, safe, finalized })
    }
}
