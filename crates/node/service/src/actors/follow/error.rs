use thiserror::Error;

/// The error type for the `FollowActor`.
#[derive(Error, Debug)]
pub enum FollowActorError {
    /// Error from the follow client.
    #[error("Follow client error: {0}")]
    FollowClient(#[from] crate::FollowClientError),
}
