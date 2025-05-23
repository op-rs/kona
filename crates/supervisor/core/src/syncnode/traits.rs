//! Traits for synchronization node functionality.
//!
//! This module defines the core traits that represent the functionality of a synchronization
//! node in the system. It includes error types and interfaces for node synchronization,
//! control operations, and event subscription.

use thiserror::Error;

/// Errors that can occur during node subscription and synchronization operations.
#[derive(Debug, Error)]
pub enum SubscriptionError {
    /// Error related to JSON Web Token handling
    #[error("jwt error: {0}")]
    JwtError(String),

    /// Authentication failures when connecting to the node
    #[error("authentication error: {0}")]
    Auth(String),

    /// Errors from the publish-subscribe system
    #[error("pubSub error: {0}")]
    PubSub(String),

    /// Errors when attempting to attach to a node
    #[error("attaching Node error: {0}")]
    AttachNodeError(String),
}
