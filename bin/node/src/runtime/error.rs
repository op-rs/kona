//! Runtime loader error type.

use alloy_transport::{RpcError, TransportErrorKind};

/// Error type for the runtime loader.
#[derive(thiserror::Error, Debug)]
pub enum RuntimeLoaderError {
    /// Transport error
    #[error(transparent)]
    Transport(#[from] RpcError<TransportErrorKind>),
}
