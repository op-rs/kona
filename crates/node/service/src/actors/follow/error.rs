use thiserror::Error;

/// The error type for the `FollowActor`.
#[derive(Error, Debug)]
pub enum FollowActorError {
    /// Error from the follow client.
    #[error("Follow client error: {0}")]
    FollowClient(#[from] crate::FollowClientError),

    /// Channel closed error.
    #[error("Channel closed: {0}")]
    ChannelClosed(String),

    /// L1 validation error.
    #[error("L1 validation error: {0}")]
    L1ValidationError(String),
}
