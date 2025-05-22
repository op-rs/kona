//! Traits for synchronization node functionality.
//!
//! This module defines the core traits that represent the functionality of a synchronization
//! node in the system. It includes error types and interfaces for node synchronization,
//! control operations, and event subscription.

use thiserror::Error;
// use crate::types::{BlockRef, ManagedEvent};

/// Errors that can occur during node subscription and synchronization operations.
#[derive(Debug, Error)]
pub enum SubscriptionError {
    /// Error related to JSON Web Token handling
    #[error("JWT error: {0}")]
    JwtError(String),
    
    /// Authentication failures when connecting to the node
    #[error("Authentication error: {0}")]
    Auth(String),
    
    /// Errors from the publish-subscribe system
    #[error("PubSub error: {0}")]
    PubSub(String),
    
    /// Errors when attempting to attach to a node
    #[error("Attaching Node error: {0}")]
    AttachNodeError(String),
}

// /// Defines functionality for synchronization of data from the node.
// pub trait SyncSource {
//     /// Retrieves the chain ID from the sync source.
//     ///
//     /// # Returns
//     /// - `Ok(u64)` containing the chain ID on success
//     /// - `Err(SubscriptionError)` if the operation fails
//     fn chain_id(&self) -> Result<u64, SubscriptionError>;
// }

// /// Defines control operations that the supervisor can perform on the node.
// pub trait SyncControl {
//     /// Subscribes to node events.
//     ///
//     /// # Arguments
//     /// * `event` - The event to subscribe to
//     ///
//     /// # Returns
//     /// - `Ok(())` on successful subscription
//     /// - `Err(SubscriptionError)` if the subscription fails
//     fn subscribe_events(&self, event: ManagedEvent) -> Result<(), SubscriptionError>;
    
//     /// Provides a new L1 block to the node for processing.
//     ///
//     /// # Arguments
//     /// * `next_l1` - Reference to the next L1 block to be processed
//     ///
//     /// # Returns
//     /// - `Ok(())` if the block was successfully provided
//     /// - `Err(SubscriptionError)` if an error occurs
//     fn provide_l1(&self, next_l1: BlockRef) -> Result<(), SubscriptionError>;
// }

// /// Complete synchronization node interface.
// pub trait SyncNode: SyncSource + SyncControl {}
