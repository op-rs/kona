//! [`ManagedNode`] implementation for subscribing to the events from managed node.

use alloy_eips::BlockNumberOrTag;
use alloy_network::Ethereum;
use alloy_primitives::{B256, ChainId};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_types_eth::BlockNumHash;
use async_trait::async_trait;
use kona_interop::{BlockReplacement, DerivedRefPair};
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DerivationStorageReader, HeadRefStorageReader, LogStorageReader};
use kona_supervisor_types::{BlockSeal, OutputV0, Receipts};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use super::{
    BlockProvider, ManagedNodeClient, ManagedNodeController, ManagedNodeDataProvider,
    ManagedNodeError, SubscriptionHandler, resetter::Resetter,
};
use crate::event::ChainEvent;
use tracing::{error, info, trace, warn};

/// [`ManagedNode`] handles the subscription to managed node events.
///
/// It manages the WebSocket connection lifecycle and processes incoming events.
#[derive(Debug)]
pub struct ManagedNode<DB, C> {
    /// The attached web socket client
    client: Arc<C>,
    /// Shared L1 provider for fetching receipts
    l1_provider: RootProvider<Ethereum>,
    /// Resetter for handling node resets
    resetter: Arc<Resetter<DB, C>>,
    /// Channel for sending events to the chain processor
    chain_event_sender: mpsc::Sender<ChainEvent>,

    /// Cached chain ID
    chain_id: Mutex<Option<ChainId>>,
}

impl<DB, C> ManagedNode<DB, C>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    /// Creates a new [`ManagedNode`] with the specified client.
    pub fn new(
        client: Arc<C>,
        db_provider: Arc<DB>,
        l1_provider: RootProvider<Ethereum>,
        chain_event_sender: mpsc::Sender<ChainEvent>,
    ) -> Self {
        let resetter = Arc::new(Resetter::new(client.clone(), db_provider));

        Self { client, resetter, l1_provider, chain_event_sender, chain_id: Mutex::new(None) }
    }

    /// Returns the [`ChainId`] of the [`ManagedNode`].
    /// If the chain ID is already cached, it returns that.
    /// If not, it fetches the chain ID from the managed node.
    pub async fn chain_id(&self) -> Result<ChainId, ManagedNodeError> {
        // we are caching the chain ID here to avoid multiple calls to the client
        // there is a possibility that chain ID might be being cached in the client already
        // but we are caching it here to make sure it caches in the `ManagedNode` context
        let mut cache = self.chain_id.lock().await;
        if let Some(chain_id) = *cache {
            Ok(chain_id)
        } else {
            let chain_id = self.client.chain_id().await?;
            *cache = Some(chain_id);
            Ok(chain_id)
        }
    }
}

#[async_trait]
impl<DB, C> SubscriptionHandler for ManagedNode<DB, C>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    async fn handle_exhaust_l1(
        &self,
        derived_ref_pair: &DerivedRefPair,
    ) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(
            target: "supervisor::managed_node",
            %chain_id,
            %derived_ref_pair,
            "Handling L1 exhaust event"
        );

        let next_block_number = derived_ref_pair.source.number + 1;
        let next_block = self
            .l1_provider
            .get_block_by_number(BlockNumberOrTag::Number(next_block_number))
            .await
            .map_err(|err| {
                error!(target: "supervisor::managed_node", %chain_id, %err, "Failed to fetch next L1 block");
                ManagedNodeError::GetBlockByNumberFailed(next_block_number)
            })?;

        let block = match next_block {
            Some(block) => block,
            None => {
                // If the block is None, it means the block is either empty or unavailable.
                // ignore this case
                return Ok(());
            }
        };

        let new_source = BlockInfo {
            hash: block.header.hash,
            number: block.header.number,
            parent_hash: block.header.parent_hash,
            timestamp: block.header.timestamp,
        };

        if block.header.parent_hash != derived_ref_pair.source.hash {
            // this could happen due to a reorg.
            // this case should be handled by the reorg manager
            warn!(
                target: "supervisor::managed_node",
                %chain_id,
                %new_source,
                current_source = %derived_ref_pair.source,
                "Parent hash mismatch. Possible reorg detected"
            );
            return Ok(());
        }

        self.client.provide_l1(new_source).await.inspect_err(|err| {
            error!(
                target: "supervisor::managed_node",
                %chain_id,
                %new_source,
                %err,
                "Failed to provide L1 block"
            );
        })?;
        Ok(())
    }

    async fn handle_reset(&self, reset_id: &str) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, reset_id, "Handling reset event");

        self.resetter.reset().await?;
        Ok(())
    }

    async fn handle_unsafe_block(&self, unsafe_block: &BlockInfo) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, %unsafe_block, "Unsafe block event received");

        self.chain_event_sender.send(ChainEvent::UnsafeBlock { block: *unsafe_block }).await.map_err(|err| {
            warn!(target: "supervisor::managed_node", %chain_id, %err, "Failed to send unsafe block event");
            ManagedNodeError::ChannelSendFailed(err.to_string())
        })?;
        Ok(())
    }

    async fn handle_derivation_update(
        &self,
        derived_ref_pair: &DerivedRefPair,
    ) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, "Derivation update event received");

        self.chain_event_sender.send(ChainEvent::DerivedBlock { derived_ref_pair: *derived_ref_pair }).await.map_err(|err| {
            warn!(target: "supervisor::managed_node", %chain_id, %err, "Failed to send derivation update event");
            ManagedNodeError::ChannelSendFailed(err.to_string())
        })?;
        Ok(())
    }

    async fn handle_replace_block(
        &self,
        replacement: &BlockReplacement,
    ) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, %replacement, "Block replacement received");

        self.chain_event_sender.send(ChainEvent::BlockReplaced { replacement: *replacement }).await.map_err(|err| {
            warn!(target: "supervisor::managed_node", %chain_id, %err, "Failed to send block replacement event");
            ManagedNodeError::ChannelSendFailed(err.to_string())
        })?;
        Ok(())
    }

    async fn handle_derivation_origin_update(
        &self,
        origin: &BlockInfo,
    ) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, %origin, "Derivation origin update received");

        self.chain_event_sender.send(ChainEvent::DerivationOriginUpdate { origin: *origin }).await.map_err(|err| {
            warn!(target: "supervisor::managed_node", %chain_id, %err, "Failed to send derivation origin update event");
            ManagedNodeError::ChannelSendFailed(err.to_string())
        })?;
        Ok(())
    }
}

/// Implements [`BlockProvider`] for [`ManagedNode`] by delegating to the underlying WebSocket
/// client.
#[async_trait]
impl<DB, C> BlockProvider for ManagedNode<DB, C>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    async fn block_by_number(&self, block_number: u64) -> Result<BlockInfo, ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, block_number, "Fetching block by number");

        let block = self.client.block_ref_by_number(block_number).await?;
        Ok(block)
    }
    async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, %block_hash, "Fetching receipts for block");

        let receipt = self.client.fetch_receipts(block_hash).await?;
        Ok(receipt)
    }
}

#[async_trait]
impl<DB, C> ManagedNodeDataProvider for ManagedNode<DB, C>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, timestamp, "Fetching output v0 at timestamp");

        let outputv0 = self.client.output_v0_at_timestamp(timestamp).await?;
        Ok(outputv0)
    }

    async fn pending_output_v0_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<OutputV0, ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, timestamp, "Fetching pending output v0 at timestamp");

        let outputv0 = self.client.pending_output_v0_at_timestamp(timestamp).await?;
        Ok(outputv0)
    }

    async fn l2_block_ref_by_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<BlockInfo, ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, timestamp, "Fetching L2 block ref by timestamp");

        let block = self.client.l2_block_ref_by_timestamp(timestamp).await?;
        Ok(block)
    }
}

#[async_trait]
impl<DB, C> ManagedNodeController for ManagedNode<DB, C>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    async fn update_finalized(
        &self,
        finalized_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(
            target: "supervisor::managed_node",
            %chain_id,
            finalized_block_number = finalized_block_id.number,
            "Updating finalized block"
        );

        self.client.update_finalized(finalized_block_id).await?;
        Ok(())
    }

    async fn update_cross_unsafe(
        &self,
        cross_unsafe_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(
            target: "supervisor::managed_node",
            %chain_id,
            cross_unsafe_block_number = cross_unsafe_block_id.number,
            "Updating cross unsafe block",
        );

        self.client.update_cross_unsafe(cross_unsafe_block_id).await?;
        Ok(())
    }

    async fn update_cross_safe(
        &self,
        source_block_id: BlockNumHash,
        derived_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(
            target: "supervisor::managed_node",
            %chain_id,
            source_block_number = source_block_id.number,
            derived_block_number = derived_block_id.number,
            "Updating cross safe block"
        );
        self.client.update_cross_safe(source_block_id, derived_block_id).await?;
        Ok(())
    }

    async fn reset(&self) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        trace!(target: "supervisor::managed_node", %chain_id, "Resetting managed node state");

        self.resetter.reset().await?;
        Ok(())
    }

    async fn invalidate_block(&self, block_seal: BlockSeal) -> Result<(), ManagedNodeError> {
        let chain_id = self.chain_id().await?;
        info!(
            target: "supervisor::managed_node",
            %chain_id,
            block_number = block_seal.number,
            "Invalidating block"
        );

        self.client.invalidate_block(block_seal).await?;
        Ok(())
    }
}
