use alloy_primitives::B256;
use thiserror::Error;

/// Represents various errors that can occur during node management,
#[derive(Debug, Error)]
pub enum ManagedNodeError {
    /// Represents an error that occurred while starting the managed node.
    #[error(transparent)]
    Client(#[from] jsonrpsee::core::ClientError),

    /// Represents an error that occurred while subscribing to the managed node.
    #[error("subscription error: {0}")]
    Subscription(#[from] SubscriptionError),

    /// Represents an error that occurred while authenticating to the managed node.
    #[error("failed to authenticate: {0}")]
    Authentication(#[from] AuthenticationError),
}

/// Error establishing authenticated connection to managed node.
#[derive(Debug, Error)]
pub enum AuthenticationError {
    /// Missing valid JWT secret for authentication header.
    #[error("jwt secret not found or invalid")]
    InvalidJwt,
    /// Invalid header format.
    #[error("invalid authorization header")]
    InvalidHeader,
}

/// Error subscribing to managed node.
#[derive(Debug, Error)]
pub enum SubscriptionError {
    /// Subscription is already exists.
    #[error("subscription already active")]
    AlreadyActive,
    /// Failure sending stop signal to managed mode daemon.
    #[error("failed to send stop signal")]
    SendStopSignalFailed,
    /// No channel found for sending stop signal.
    #[error("no active stop channel")]
    MissingStopChannel,
    /// Failure shutting down managed mode daemon.
    #[error("failed to join task")]
    ShutdownDaemonFailed,
    /// Subscription has already been stopped or wasn't active.
    #[error("subscription not active or already stopped")]
    SubscriptionNotFound,
}

/// Error handling managed event task.
#[derive(Debug, Error, PartialEq)]
pub enum ManagedEventTaskError {
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
    /// This should never happen, new() always sets the rpc client when creating the task.
    #[error("rpc client for managed node is not set")]
    ManagedNodeClientMissing,
    /// Managed node api call failed.
    #[error("managed node api call failed")]
    ManagedNodeAPICallFailed,
    /// Next block is either empty or unavailable.
    #[error("next block is either empty or unavailable, number: {0}")]
    NextBlockNotFound(u64),
}
