use crate::syncnode::{ManagedNodeError, NodeEvent};
use async_trait::async_trait;
use kona_supervisor_types::ReceiptProvider;
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
    /// * `event_tx` - A Tokio MPSC sender through which [`NodeEvent`]s will be emitted.
    ///
    /// # Returns
    /// * `Ok(())` on successful subscription
    /// * `Err(ManagedNodeError)` if subscription setup fails
    async fn start_subscription(
        &self,
        event_tx: mpsc::Sender<NodeEvent>,
    ) -> Result<(), ManagedNodeError>;
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
