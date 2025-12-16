//! Errors for the [`FollowTask`].

use crate::{EngineTaskErrorSeverity, SynchronizeTaskError};

/// An error that occurred while executing a [`FollowTask`].
///
/// [`FollowTask`]: super::FollowTask
#[derive(Debug, thiserror::Error)]
pub enum FollowTaskError {
    /// An error occurred while synchronizing the engine state.
    #[error(transparent)]
    Synchronize(#[from] SynchronizeTaskError),
}

impl crate::EngineTaskError for FollowTaskError {
    fn severity(&self) -> EngineTaskErrorSeverity {
        match self {
            Self::Synchronize(e) => e.severity(),
        }
    }
}
