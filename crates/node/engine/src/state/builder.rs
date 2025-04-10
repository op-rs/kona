//! An [EngineState] builder.

use crate::{EngineClient, EngineState, SyncStatus, client::EngineClientError};
use alloy_eips::eip1898::BlockNumberOrTag;
use kona_genesis::ChainGenesis;
use thiserror::Error;

use kona_protocol::L2BlockInfo;

/// An error that occurs in the [EngineStateBuilder].
#[derive(Error, Debug)]
pub enum EngineStateBuilderError {
    /// A temporary error within the engine.
    #[error("Temporary engine task error: {0}")]
    EngineClientError(#[from] EngineClientError),

    /// An error that occurs when wrongly configuring the engine.
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// A builder for the [EngineState].
///
/// When the [EngineState] is first created, only the finalized
/// block is specified. The `StateBuilder` constructs the
/// [EngineState] by fetching the remaining block info via the
/// client.
#[derive(Debug, Clone)]
pub struct EngineStateBuilder {
    /// The engine client.
    client: EngineClient,
    /// The chain genesis.
    genesis: ChainGenesis,
    /// The sync status of the engine.
    sync_status: Option<SyncStatus>,
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

impl EngineStateBuilder {
    /// Constructs a new [EngineStateBuilder] from the provided client.
    pub const fn new(client: EngineClient, genesis: ChainGenesis) -> Self {
        Self {
            client,
            genesis,
            sync_status: None,
            unsafe_head: None,
            cross_unsafe_head: None,
            pending_safe_head: None,
            local_safe_head: None,
            safe_head: None,
            finalized_head: None,
        }
    }

    /// Fetches the unsafe head block info if it is not already set.
    async fn fetch_unsafe_head(&mut self) -> Result<&mut Self, EngineStateBuilderError> {
        if self.unsafe_head.is_none() {
            self.unsafe_head =
                self.client.l2_block_info_by_label(BlockNumberOrTag::Pending).await?;
        }
        Ok(self)
    }

    /// Fetches the safe head block info if it is not already set.
    async fn fetch_safe_head(&mut self) -> Result<&mut Self, EngineStateBuilderError> {
        if self.safe_head.is_none() {
            self.safe_head = match self.client.l2_block_info_by_label(BlockNumberOrTag::Safe).await
            {
                Ok(Some(safe_head)) => Some(safe_head),
                Ok(None) => {
                    debug!(target: "engine", "No safe head, using genesis");
                    self.finalized_head
                }
                Err(e) => return Err(e.into()),
            };
        }
        Ok(self)
    }

    /// Fetches the finalized head block info if it is not already set.
    async fn fetch_finalized_head(&mut self) -> Result<&mut Self, EngineStateBuilderError> {
        if self.finalized_head.is_none() {
            match self.client.l2_block_info_by_label(BlockNumberOrTag::Finalized).await {
                Ok(Some(finalized_head)) => {
                    self.finalized_head = Some(finalized_head);
                }
                Ok(None) => {
                    debug!(target: "engine", "No finalized head, using genesis");
                    self.finalized_head =
                        self.client.l2_block_info_by_label(self.genesis.l2.number.into()).await?;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(self)
    }

    /// Append the sync status to the [EngineStateBuilder].
    pub fn with_sync_status(&mut self, sync_status: SyncStatus) -> &mut Self {
        self.sync_status = Some(sync_status);
        self
    }

    /// Builds the [EngineState], fetching missing block info if necessary.
    pub async fn build(self) -> Result<EngineState, EngineStateBuilderError> {
        let mut builder = self;
        debug!(target: "engine", "Building engine state");
        builder.fetch_unsafe_head().await?;
        debug!(target: "engine", "Fetched unsafe head");
        builder.fetch_finalized_head().await?;
        debug!(target: "engine", "Fetched finalized head");
        builder.fetch_safe_head().await?;
        debug!(target: "engine", "Fetched safe head");

        let unsafe_head = if let Some(h) = builder.unsafe_head {
            h
        } else {
            return Err(EngineStateBuilderError::ConfigError(
                "unsafe_head is required to build the EngineState".into(),
            ));
        };

        let finalized_head = if let Some(h) = builder.finalized_head {
            h
        } else {
            return Err(EngineStateBuilderError::ConfigError(
                "finalized_head is required to build the EngineState".into(),
            ));
        };

        let safe_head = if let Some(h) = builder.safe_head {
            h
        } else {
            return Err(EngineStateBuilderError::ConfigError(
                "safe_head is required to build the EngineState".into(),
            ));
        };

        let local_safe_head = builder.local_safe_head.unwrap_or(safe_head);
        let cross_unsafe_head = builder.cross_unsafe_head.unwrap_or(safe_head);
        let pending_safe_head = builder.pending_safe_head.unwrap_or(safe_head);

        Ok(EngineState {
            sync_status: builder.sync_status.unwrap_or_default(),
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
