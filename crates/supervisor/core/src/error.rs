//! [`SupervisorService`](crate::SupervisorService) errors.

use crate::{ChainProcessorError, CrossSafetyError, syncnode::ManagedNodeError};
use jsonrpsee::types::{ErrorCode, ErrorObjectOwned};
use kona_supervisor_storage::StorageError;
use kona_supervisor_types::AccessListError;
use op_alloy_rpc_types::SuperchainDAError;
use thiserror::Error;

/// Custom error type for the Supervisor core logic.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SupervisorError {
    /// Indicates that a feature or method is not yet implemented.
    #[error("functionality not implemented")]
    Unimplemented,
    /// No chains are configured for supervision.
    #[error("empty dependency set")]
    EmptyDependencySet,
    /// Data availability errors.
    ///
    /// Spec <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#protocol-specific-error-codes>.
    #[error(transparent)]
    DataAvailability(#[from] SuperchainDAError),

    /// Indicates that the supervisor was unable to initialise due to an error.
    #[error("unable to initialize the supervisor: {0}")]
    Initialise(String),

    /// Indicates that error occurred while interacting with the storage layer.
    #[error(transparent)]
    StorageError(#[from] StorageError),

    /// Indicates the error occurred while interacting with the managed node.
    #[error(transparent)]
    ManagedNodeError(#[from] ManagedNodeError),

    /// Indicates the error occurred while processing the chain.
    #[error(transparent)]
    ChainProcessorError(#[from] ChainProcessorError),

    /// Indicates the error occurred while parsing the access_list
    #[error(transparent)]
    AccessListError(#[from] AccessListError),

    /// Indicated the error occurred while initializing the safety checker jobs
    #[error(transparent)]
    CrossSafetyCheckerError(#[from] CrossSafetyError),

    /// Indicates the L1 block does not match the epxected L1 block.
    #[error("L1 block number mismatch. expected: {expected}, but got {got}")]
    L1BlockMismatch {
        /// Expected L1 block.
        expected: u64,
        /// Received L1 block.
        got: u64,
    },
}

impl From<SupervisorError> for ErrorObjectOwned {
    fn from(err: SupervisorError) -> Self {
        match err {
            // todo: handle these errors more gracefully
            SupervisorError::Unimplemented |
            SupervisorError::EmptyDependencySet |
            SupervisorError::L1BlockMismatch { .. } |
            SupervisorError::Initialise(_) |
            SupervisorError::StorageError(_) |
            SupervisorError::ManagedNodeError(_) |
            SupervisorError::ChainProcessorError(_) |
            SupervisorError::CrossSafetyCheckerError(_) |
            SupervisorError::AccessListError(_) => ErrorObjectOwned::from(ErrorCode::InternalError),
            SupervisorError::DataAvailability(err) => err.into(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_rpc_error_conversion() {
        let err = SuperchainDAError::UnknownChain;
        let rpc_err = ErrorObjectOwned::owned(err as i32, err.to_string(), None::<()>);

        assert_eq!(ErrorObjectOwned::from(SupervisorError::DataAvailability(err)), rpc_err);
    }
}
