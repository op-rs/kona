//! The [`L1OriginSelector`].

use alloy_eips::BlockId;
use alloy_provider::{Provider, RootProvider};
use alloy_transport::{RpcError, TransportErrorKind};
use kona_genesis::RollupConfig;
use kona_protocol::{BlockInfo, L2BlockInfo};
use std::sync::Arc;

/// The [`L1OriginSelector`] is responsible for selecting the L1 origin block based on the
/// current L2 unsafe head's sequence epoch.
#[derive(Debug)]
pub struct L1OriginSelector {
    /// The [`RollupConfig`].
    cfg: Arc<RollupConfig>,
    /// The L1 [`RootProvider`].
    l1: RootProvider,
    /// The current L1 origin.
    current: Option<BlockInfo>,
    /// The next L1 origin.
    next: Option<BlockInfo>,
}

impl L1OriginSelector {
    /// Creates a new [`L1OriginSelector`].
    pub const fn new(cfg: Arc<RollupConfig>, l1: RootProvider) -> Self {
        Self { cfg, l1, current: None, next: None }
    }

    /// Returns the current L1 origin.
    pub const fn current(&self) -> Option<&BlockInfo> {
        self.current.as_ref()
    }

    /// Returns the next L1 origin.
    pub const fn next(&self) -> Option<&BlockInfo> {
        self.next.as_ref()
    }

    /// Determines what the next L1 origin block should be, based off of the [`L2BlockInfo`] unsafe
    /// head.
    ///
    /// The L1 origin is selected based off of the sequencing epoch, determined by the next L2
    /// block's timestamp in relation to the current L1 origin's timestamp. If the next L2
    /// block's timestamp is greater than the L2 unsafe head's L1 origin timestamp, the L1
    /// origin is the block following the current L1 origin.
    pub async fn next_l1_origin(
        &mut self,
        unsafe_head: L2BlockInfo,
    ) -> Result<BlockInfo, L1OriginSelectorError> {
        self.select_origins(&unsafe_head).await?;

        let (current, mut next) = (self.current, self.next);

        // Start building on the next L1 origin block if the next L2 block's timestamp is
        // greater than or equal to the next L1 origin's timestamp.
        if let Some(next) = next {
            if unsafe_head.block_info.timestamp + self.cfg.block_time >= next.timestamp {
                return Ok(next);
            }
        }

        let Some(current) = current else {
            unreachable!("Current L1 origin should always be set by `select_origins`");
        };

        let max_seq_drift = self.cfg.max_sequencer_drift(current.timestamp);
        let past_seq_drift = unsafe_head.block_info.timestamp + self.cfg.block_time -
            current.timestamp >
            max_seq_drift;

        // If the sequencer drift has not been exceeded, return the current L1 origin.
        if !past_seq_drift {
            return Ok(current);
        }

        let next_block_number = current.number.saturating_add(1);

        // If the next L1 origin is not set, fetch the next block after the current L1 origin.
        if self.next.is_none() {
            let next_block = self
                .l1
                .get_block_by_number(next_block_number.into())
                .await?
                .ok_or(L1OriginSelectorError::BlockNotFound(next_block_number.into()))?
                .into();

            next = Some(next_block);
        }

        warn!(
            target: "l1_origin_selector",
            current_origin_time = current.timestamp,
            unsafe_head_time = unsafe_head.block_info.timestamp,
            max_seq_drift,
            "Next L2 block time is past the sequencer drift"
        );

        if next
            .map(|n| unsafe_head.block_info.timestamp + self.cfg.block_time < n.timestamp)
            .unwrap_or(false)
        {
            // If the next L2 block's timestamp is less than the next L1 origin's timestamp,
            // return the current L1 origin.
            return Ok(current);
        }

        next.ok_or(L1OriginSelectorError::BlockNotFound(next_block_number.into()))
    }

    /// Selects the current and next L1 origin blocks based on the unsafe head.
    async fn select_origins(
        &mut self,
        unsafe_head: &L2BlockInfo,
    ) -> Result<(), L1OriginSelectorError> {
        if self.current.map(|c| c.hash == unsafe_head.l1_origin.hash).unwrap_or(false) {
            // Do nothing; The next L2 block exists in the same epoch as the current L1 origin.
        } else if self.next.map(|n| n.hash == unsafe_head.l1_origin.hash).unwrap_or(false) {
            // Advance the origin.
            self.current = self.next.take();
            self.next = None;
        } else {
            // Find the current origin block, as it is missing.
            let current: BlockInfo = self
                .l1
                .get_block_by_hash(unsafe_head.l1_origin.hash)
                .await?
                .ok_or(L1OriginSelectorError::BlockNotFound(unsafe_head.l1_origin.hash.into()))?
                .into();

            self.current = Some(current);
            self.next = None;
        }

        Ok(())
    }
}

/// An error produced by the [`L1OriginSelector`].
#[derive(Debug, thiserror::Error)]
pub enum L1OriginSelectorError {
    /// An error produced by the [`RootProvider`].
    #[error(transparent)]
    Provider(#[from] RpcError<TransportErrorKind>),
    /// A block could not be found.
    #[error("Block {0} could not be found")]
    BlockNotFound(BlockId),
}
