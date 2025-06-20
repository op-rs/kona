use super::ManagedNodeError;
use crate::event::ChainEvent;
use alloy_primitives::B256;
use async_trait::async_trait;
use kona_protocol::BlockInfo;
use kona_supervisor_types::{OutputV0, Receipts};
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

/// [`ManagedNodeApiProvider`] abstracts the managed node APIs that supervisor uses to fetch info
/// from the managed node.
#[async_trait]
pub trait ManagedNodeApiProvider: Send + Sync + Debug {
    /// Fetch the output v0 at a given timestamp.
    ///
    /// # Arguments
    /// * `timestamp` - The timestamp to fetch the output v0 at.
    ///
    /// # Returns
    /// The output v0 at the given timestamp,
    /// or an error if the fetch fails.
    async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ManagedNodeError>;

    /// Fetch the pending output v0 at a given timestamp.
    ///
    /// # Arguments
    /// * `timestamp` - The timestamp to fetch the pending output v0 at.
    ///
    /// # Returns
    /// The pending output v0 at the given timestamp,
    /// or an error if the fetch fails.
    async fn pending_output_v0_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<OutputV0, ManagedNodeError>;

    /// Fetch the l2 block ref by timestamp.
    ///
    /// # Arguments
    /// * `timestamp` - The timestamp to fetch the l2 block ref at.
    ///
    /// # Returns
    /// The l2 block ref at the given timestamp,
    async fn l2_block_ref_by_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<BlockInfo, ManagedNodeError>;
}

/// Composite trait for any node that provides:
/// - Event subscriptions (`NodeSubscriber`)
/// - Receipt access (`ReceiptProvider`)
/// - Managed node API access (`ManagedNodeApiProvider`)
///
/// This is the main abstraction used for a fully-managed node
/// within the supervisor context.
#[async_trait]
pub trait ManagedNodeProvider:
    NodeSubscriber + ReceiptProvider + ManagedNodeApiProvider + Send + Sync + Debug
{
}

#[async_trait]
impl<T> ManagedNodeProvider for T where
    T: NodeSubscriber + ReceiptProvider + ManagedNodeApiProvider + Send + Sync + Debug
{
}
