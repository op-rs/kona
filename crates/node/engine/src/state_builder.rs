//! An [EngineState] builder.

use alloy_eips::eip1898::BlockNumberOrTag;
use anyhow::{Result, bail};
use kona_protocol::L2BlockInfo;

use crate::{EngineClient, EngineState};

/// A builder for the [EngineState].
///
/// When the [EngineState] is first created, only the finalized
/// block is specified. The `StateBuilder` constructs the
/// [EngineState] by fetching the remaining block info via the
/// [EngineClient].
#[derive(Debug, Clone)]
pub struct StateBuilder {
    /// The engine client.
    client: EngineClient,
    /// Most recent block found on the p2p network
    unsafe_head: Option<L2BlockInfo>,
    /// Cross-verified unsafe head, always equal to the unsafe head pre-interop
    cross_unsafe_head: Option<L2BlockInfo>,
    /// Pending localSafeHead
    /// L2 block processed from the middle of a span batch,
    /// but not marked as the safe block yet.
    pending_safe_head: Option<L2BlockInfo>,
    /// Derived from L1, and known to be a completed span-batch,
    /// but not cross-verified yet.
    local_safe_head: Option<L2BlockInfo>,
    /// Derived from L1 and cross-verified to have cross-safe dependencies.
    safe_head: Option<L2BlockInfo>,
    /// Derived from finalized L1 data,
    /// and cross-verified to only have finalized dependencies.
    finalized_head: Option<L2BlockInfo>,
}

impl StateBuilder {
    /// Constructs a new [StateBuilder] from the provided [EngineClient].
    pub const fn new(client: EngineClient) -> Self {
        Self {
            client,
            unsafe_head: None,
            cross_unsafe_head: None,
            pending_safe_head: None,
            local_safe_head: None,
            safe_head: None,
            finalized_head: None,
        }
    }

    /// Fetches the unsafe head block info if it is not already set.
    async fn fetch_unsafe_head(&mut self) -> Result<&mut Self> {
        if self.unsafe_head.is_none() {
            self.unsafe_head =
                Some(self.client.l2_block_info_by_label(BlockNumberOrTag::Pending).await?);
        }
        Ok(self)
    }

    /// Fetches the finalized head block info if it is not already set.
    async fn fetch_finalized_head(&mut self) -> Result<&mut Self> {
        if self.finalized_head.is_none() {
            self.finalized_head =
                Some(self.client.l2_block_info_by_label(BlockNumberOrTag::Finalized).await?);
        }
        Ok(self)
    }

    /// Fetches the safe head block info if it is not already set.
    async fn fetch_safe_head(&mut self) -> Result<&mut Self> {
        if self.safe_head.is_none() {
            let safe_head = match self.client.l2_block_info_by_label(BlockNumberOrTag::Safe).await {
                Ok(safe_head) => Some(safe_head),
                // TODO: only set this if the block is not found
                // SEE: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/rollup/engine/engine_controller.go#L293
                Err(_) => self.finalized_head,
            };
            self.safe_head = safe_head;
        }
        Ok(self)
    }

    /// Builds the [EngineState], fetching missing block info if necessary.
    pub async fn build(self) -> Result<EngineState> {
        let mut builder = self;
        builder.fetch_unsafe_head().await?;
        builder.fetch_finalized_head().await?;
        builder.fetch_safe_head().await?;

        let unsafe_head = if let Some(h) = builder.unsafe_head {
            h
        } else {
            bail!("unsafe_head is required to build the EngineState");
        };

        let cross_unsafe_head = if let Some(h) = builder.cross_unsafe_head {
            h
        } else {
            bail!("cross_unsafe_head is required to build the EngineState");
        };

        let pending_safe_head = if let Some(h) = builder.pending_safe_head {
            h
        } else {
            bail!("pending_safe_head is required to build the EngineState");
        };

        let local_safe_head = if let Some(h) = builder.local_safe_head {
            h
        } else {
            bail!("local_safe_head is required to build the EngineState");
        };

        let safe_head = if let Some(h) = builder.safe_head {
            h
        } else {
            bail!("safe_head is required to build the EngineState");
        };

        let finalized_head = if let Some(h) = builder.finalized_head {
            h
        } else {
            bail!("finalized_head is required to build the EngineState");
        };

        Ok(EngineState {
            unsafe_head,
            cross_unsafe_head,
            pending_safe_head,
            local_safe_head,
            safe_head,
            finalized_head,
            backup_unsafe_head: None,
            forkchoice_update_needed: false,
            need_fcu_call_backup_unsafe_reorg: false,
        })
    }
}
