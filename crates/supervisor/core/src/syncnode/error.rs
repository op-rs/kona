use thiserror::Error;

/// Represents various errors that can occur during node management,
#[derive(Debug, Error)]
pub enum ManagedNodeError {
    /// Represents an error that occurred while starting the managed node.
    #[error("webSocket connection failed: {0}")]
    Client(#[from] jsonrpsee::core::ClientError),

    /// Represents an error that occurred while subscribing to the managed node.
    #[error("subscription error: {0}")]
    Subscription(String),

    /// Represents an error that occurred while authenticating to the managed node.
    #[error("failed to authenticate: {0}")]
    Authentication(String),
}
