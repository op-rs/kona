//! Error type when building the discovery service.

use derive_more::From;
use thiserror::Error;

/// An error that can occur when building the discovery service.
#[derive(Debug, Clone, Copy, PartialEq, From, Eq, Error)]
pub enum Discv5BuilderError {
    /// The chain ID is not set.
    #[error("chain ID not set")]
    ChainIdNotSet,
    /// The listen config is not set.
    #[error("listen config not set")]
    ListenConfigNotSet,
    /// Could not create the discovery service.
    #[error("could not create discovery service")]
    Discv5CreationFailed,
    /// Failed to build the ENR.
    #[error("failed to build ENR")]
    EnrBuildFailed,
}
