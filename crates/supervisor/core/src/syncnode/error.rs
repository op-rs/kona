use thiserror::Error;

/// Represents various errors that can occur during node management,
#[derive(Debug, Error)]
pub enum ManagedNodeError {
    /// Represents an error that occurred while starting the managed node.
    #[error(transparent)]
    Client(#[from] jsonrpsee::core::ClientError),

    /// Represents an error that occurred while subscribing to the managed node.
    #[error("subscription error: {0}")]
    Subscription(String),

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
