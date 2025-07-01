//! [`ManagedNode`] implementation for subscribing to the events from managed node.

use alloy_network::Ethereum;
use alloy_primitives::{B256, ChainId};
use alloy_provider::RootProvider;
use alloy_rpc_types_eth::BlockNumHash;
use async_trait::async_trait;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DerivationStorageReader, HeadRefStorageReader, LogStorageReader};
use kona_supervisor_types::{OutputV0, Receipts};
use std::sync::Arc;
use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::{
    ManagedEventTask, ManagedNodeApiProvider, ManagedNodeClient, ManagedNodeError, NodeSubscriber,
    ReceiptProvider, SubscriptionError,
};
use crate::event::ChainEvent;

/// [`ManagedNode`] handles the subscription to managed node events.
///
/// It manages the WebSocket connection lifecycle and processes incoming events.
#[derive(Debug)]
pub struct ManagedNode<DB, C> {
    /// The attached web socket client
    client: Arc<C>,
    /// The database provider for fetching information
    db_provider: Arc<DB>,
    // Cancellation token to stop the processor
    cancel_token: CancellationToken,
    /// Handle to the async subscription task
    task_handle: Mutex<Option<JoinHandle<()>>>,
    /// Shared L1 provider for fetching receipts
    l1_provider: RootProvider<Ethereum>,
}

impl<DB, C> ManagedNode<DB, C>
where
    DB: LogStorageReader + DerivationStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    /// Creates a new [`ManagedNode`] with the specified client.
    pub fn new(
        client: Arc<C>,
        db_provider: Arc<DB>,
        cancel_token: CancellationToken,
        l1_provider: RootProvider<Ethereum>,
    ) -> Self {
        Self { client, db_provider, cancel_token, task_handle: Mutex::new(None), l1_provider }
    }

    /// Returns the [`ChainId`] of the [`ManagedNode`].
    /// If the chain ID is already cached, it returns that.
    /// If not, it fetches the chain ID from the managed node.
    pub async fn chain_id(&self) -> Result<ChainId, ManagedNodeError> {
        self.client.chain_id().await
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

        let mut subscription = self.client.subscribe_events().await.inspect_err(|err| {
            error!(
                target: "managed_node",
                %err,
                "Failed to subscribe to events"
            );
        })?;

        let cancel_token = self.cancel_token.clone();

        // Creates a task instance to sort and process the events from the subscription
        let task = ManagedEventTask::new(
            self.client.clone(),
            self.l1_provider.clone(),
            self.db_provider.clone(),
            event_tx,
        );
        // Start background task to handle events
        let handle = tokio::spawn(async move {
            info!(target: "managed_node", "Subscription task started");
            loop {
                tokio::select! {
                    // Listen for stop signal
                    _ = cancel_token.cancelled() => {
                        info!(target: "managed_node", "Cancellation token triggered, shutting down subscription");
                        break;
                    }

                    // Listen for events from subscription
                    incoming_event = subscription.next() => {
                        match incoming_event {
                            Some(Ok(subscription_event)) => {
                                task.handle_managed_event(subscription_event.data).await;
                            }
                            Some(Err(err)) => {
                                error!(
                                    target: "managed_node",
                                    %err,
                                    "Error in event deserialization"
                                );
                        // Continue processing next events despite this error
                            }
                            None => {
                                // Subscription closed by the server
                                warn!(target: "managed_node", "Subscription closed by server");
                                break;
                            }
                        }
                    }
                }
            }

            // Try to unsubscribe gracefully
            if let Err(err) = subscription.unsubscribe().await {
                warn!(
                    target: "managed_node",
                    %err,
                    "Failed to unsubscribe gracefully"
                );
            }

            info!(target: "managed_node", "Subscription task finished");
        });

        *task_handle_guard = Some(handle);

        info!(target: "managed_node", "Subscription started successfully");
        Ok(())
    }
}

/// Implements [`ReceiptProvider`] for [`ManagedNode`] by delegating to the underlying WebSocket
/// client.
#[async_trait]
impl<DB, C> ReceiptProvider for ManagedNode<DB, C>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ManagedNodeError> {
        self.client.fetch_receipts(block_hash).await
    }
}

#[async_trait]
impl<DB, C> ManagedNodeApiProvider for ManagedNode<DB, C>
where
    DB: LogStorageReader + DerivationStorageReader + HeadRefStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ManagedNodeError> {
        self.client.output_v0_at_timestamp(timestamp).await
    }

    async fn pending_output_v0_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<OutputV0, ManagedNodeError> {
        self.client.pending_output_v0_at_timestamp(timestamp).await
    }

    async fn l2_block_ref_by_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<BlockInfo, ManagedNodeError> {
        self.client.l2_block_ref_by_timestamp(timestamp).await
    }

    async fn update_finalized(
        &self,
        finalized_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError> {
        self.client.update_finalized(finalized_block_id).await
    }

    async fn update_cross_unsafe(
        &self,
        cross_unsafe_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError> {
        self.client.update_cross_unsafe(cross_unsafe_block_id).await
    }

    async fn update_cross_safe(
        &self,
        source_block_id: BlockNumHash,
        derived_block_id: BlockNumHash,
    ) -> Result<(), ManagedNodeError> {
        self.client.update_cross_safe(source_block_id, derived_block_id).await
    }
}
