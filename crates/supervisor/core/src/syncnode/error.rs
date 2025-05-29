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
    Authentication(String),
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
