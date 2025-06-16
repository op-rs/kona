use super::ManagedNodeError;
use crate::event::ChainEvent;
use alloy_primitives::B256;
use async_trait::async_trait;
use kona_supervisor_types::Receipts;
use std::fmt::Debug;
use tokio::sync::mpsc;

/// Represents a node that can subscribe to L2 events from the chain.
///
/// This trait is responsible for setting up event subscriptions and
/// streaming them through a Tokio MPSC channel. Must be thread-safe.
#[async_trait]
pub trait NodeSubscriber: Send + Sync {
    /// Starts a subscription to the node's event stream.
    ///
    /// # Arguments
    /// * `event_tx` - A Tokio MPSC sender through which [`ChainEvent`]s will be emitted.
    ///
    /// # Returns
    /// * `Ok(())` on successful subscription
    /// * `Err(ManagedNodeError)` if subscription setup fails
    async fn start_subscription(
        &self,
        event_tx: mpsc::Sender<ChainEvent>,
    ) -> Result<(), ManagedNodeError>;
}

/// [`ReceiptProvider`] abstracts fetching receipts for a given block hash.
///
/// This trait exists to decouple the indexing logs from tightly coupling with
/// the full `ManagedModeApi`. It allows using a minimal provider
/// focused only on receipt access.
#[async_trait]
pub trait ReceiptProvider: Send + Sync + Debug {
    /// Fetch all transaction receipts for the block with the given hash.
    ///
    /// # Arguments
    /// * `block_hash` - The hash of the block whose receipts should be fetched.
    ///
    /// # Returns
    /// [Receipts] representing all transaction receipts in the block,
    /// or an error if the fetch fails.
    async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ManagedNodeError>;
}

/// Composite trait for any node that provides:
/// - Event subscriptions (`NodeSubscriber`)
/// - Receipt access (`ReceiptProvider`)
///
/// This is the main abstraction used for a fully-managed node
/// within the supervisor context.
#[async_trait]
pub trait ManagedNodeProvider: NodeSubscriber + ReceiptProvider + Send + Sync + Debug {}

#[async_trait]
impl<T> ManagedNodeProvider for T where T: NodeSubscriber + ReceiptProvider + Send + Sync + Debug {}
