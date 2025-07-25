//! [`ManagedNode`] implementation for subscribing to the events from managed node.

use alloy_network::Ethereum;
use alloy_primitives::{B256, ChainId};
use alloy_provider::RootProvider;
use alloy_rpc_types_eth::BlockNumHash;
use async_trait::async_trait;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DerivationStorageReader, HeadRefStorageReader, LogStorageReader};
use kona_supervisor_types::{BlockSeal, OutputV0, Receipts};
use std::sync::Arc;
use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use super::{
    BlockProvider, ManagedNodeClient, ManagedNodeController, ManagedNodeDataProvider,
    ManagedNodeError, NodeSubscriber, SubscriptionError, resetter::Resetter,
    task::ManagedEventTask, utils::spawn_task_with_retry,
};
use crate::event::ChainEvent;

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
    /// Cancellation token to stop the processor
    cancel_token: CancellationToken,
    /// Handle to the async subscription task
    task_handle: Mutex<Option<JoinHandle<()>>>,
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
        cancel_token: CancellationToken,
        l1_provider: RootProvider<Ethereum>,
    ) -> Self {
        let resetter = Arc::new(Resetter::new(client.clone(), db_provider));

        Self { client, resetter, cancel_token, task_handle: Mutex::new(None), l1_provider }
    }

    /// Returns the [`ChainId`] of the [`ManagedNode`].
    /// If the chain ID is already cached, it returns that.
    /// If not, it fetches the chain ID from the managed node.
    pub async fn chain_id(&self) -> Result<ChainId, ManagedNodeError> {
        let chain_id = self.client.chain_id().await?;
        Ok(chain_id)
    }
}

#[async_trait]
impl<DB, C> NodeSubscriber for ManagedNode<DB, C>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    /// Starts a subscription to the managed node.
    ///
    /// Establishes a WebSocket connection and subscribes to node events.
    /// Spawns a background task to process incoming events.
    async fn start_subscription(
        &self,
        event_tx: mpsc::Sender<ChainEvent>,
    ) -> Result<(), ManagedNodeError> {
        let mut task_handle_guard = self.task_handle.lock().await;
        if task_handle_guard.is_some() {
            Err(SubscriptionError::AlreadyActive)?
        }

        let client = self.client.clone();
        let l1_provider = self.l1_provider.clone();
        let resetter = self.resetter.clone();
        let cancel_token = self.cancel_token.clone();

        // spawn a task which will be retried in failures
        let handle = spawn_task_with_retry(
            move || {
                let task = ManagedEventTask::new(
                    client.clone(),
                    l1_provider.clone(),
                    resetter.clone(),
                    cancel_token.clone(),
                    event_tx.clone(),
                );
                async move { task.run().await }
            },
            self.cancel_token.clone(),
            usize::MAX,
        );

        *task_handle_guard = Some(handle);

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
    async fn block_by_number(&self, number: u64) -> Result<BlockInfo, ManagedNodeError> {
        let block = self.client.block_ref_by_number(number).await?;
        Ok(block)
    }
    async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ManagedNodeError> {
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
        let outputv0 = self.client.output_v0_at_timestamp(timestamp).await?;
        Ok(outputv0)
    }

    async fn pending_output_v0_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<OutputV0, ManagedNodeError> {
        let outputv0 = self.client.pending_output_v0_at_timestamp(timestamp).await?;
        Ok(outputv0)
    }

    async fn l2_block_ref_by_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<BlockInfo, ManagedNodeError> {
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
        self.client.update_finalized(finalized_block_id).await?;
        Ok(())
    }

    async fn update_cross_unsafe(
        &self,
        cross_unsafe_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError> {
        self.client.update_cross_unsafe(cross_unsafe_block_id).await?;
        Ok(())
    }

    async fn update_cross_safe(
        &self,
        source_block_id: BlockNumHash,
        derived_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError> {
        self.client.update_cross_safe(source_block_id, derived_block_id).await?;
        Ok(())
    }

    async fn reset(&self) -> Result<(), ManagedNodeError> {
        self.resetter.reset().await?;
        Ok(())
    }

    async fn invalidate_block(&self, seal: BlockSeal) -> Result<(), ManagedNodeError> {
        self.client.invalidate_block(seal).await?;
        Ok(())
    }
}
