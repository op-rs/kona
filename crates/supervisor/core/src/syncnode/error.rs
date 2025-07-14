use alloy_primitives::B256;
use kona_supervisor_storage::StorageError;
use thiserror::Error;

/// Represents various errors that can occur during node management,
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManagedNodeError {
    /// Represents an error that occurred while starting the managed node.
    #[error(transparent)]
    Client(#[from] ClientError),

    /// Represents an error that occurred while subscribing to the managed node.
    #[error("subscription error: {0}")]
    Subscription(#[from] SubscriptionError),

    /// Represents an error that occurred while fetching data from the storage.
    #[error(transparent)]
    StorageError(#[from] StorageError),

    /// Represents an error that occurred while resetting the managed node.
    #[error("failed to reset the managed node")]
    ResetFailed,
}

/// Error establishing authenticated connection to managed node.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthenticationError {
    /// Missing valid JWT secret for authentication header.
    #[error("jwt secret not found or invalid")]
    InvalidJwt,
    /// Invalid header format.
    #[error("invalid authorization header")]
    InvalidHeader,
}

/// Error subscribing to managed node.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SubscriptionError {
    /// Subscription is already exists.
    #[error("subscription already active")]
    AlreadyActive,
}

/// Error handling managed event task.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManagedEventTaskError {
    /// Represents an error that occurred while starting the managed node.
    #[error(transparent)]
    Client(#[from] ClientError),

    /// Unable to successfully fetch next L1 block.
    #[error("failed to get block by number, number: {0}")]
    GetBlockByNumberFailed(u64),

    /// Current block hash and parent block hash of next block do not match.
    #[error(
        "current block hash and parent hash of next block mismatch, current: {current}, parent: {parent}"
    )]
    BlockHashMismatch {
        /// Current block hash.
        current: B256,
        /// Parent block hash of next block (which should be current block hash)
        parent: B256,
    },

    /// Managed node api call failed.
    #[error("managed node api call failed")]
    ManagedNodeAPICallFailed,

    /// Error fetching data from the storage.
    #[error(transparent)]
    StorageError(#[from] StorageError),
}

/// Represents errors that can occur while interacting with the managed node client.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Represents an error that occurred while starting the managed node.
    #[error(transparent)]
    Client(#[from] jsonrpsee::core::ClientError),

    /// Represents an error that occurred while authenticating to the managed node.
    #[error("failed to authenticate: {0}")]
    Authentication(#[from] AuthenticationError),

    /// Represents an error that occurred while parsing a chain ID from a string.
    #[error(transparent)]
    ChainIdParseError(#[from] std::num::ParseIntError),
}

impl PartialEq for ClientError {
    fn eq(&self, other: &Self) -> bool {
        use ClientError::*;
        match (self, other) {
            (Client(a), Client(b)) => a.to_string() == b.to_string(),
            (Authentication(a), Authentication(b)) => a == b,
            (ChainIdParseError(a), ChainIdParseError(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for ClientError {}
