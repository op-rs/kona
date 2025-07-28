//! [`SupervisorService`](crate::SupervisorService) errors.

use crate::{
    ChainProcessorError, CrossSafetyError, config::ConfigError, syncnode::ManagedNodeError,
};
use derive_more;
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

    /// Interop has not yet been enabled for the chain.
    #[error("interop not enabled")]
    InteropNotEnabled,

    /// Data availability errors.
    ///
    /// Spec <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#protocol-specific-error-codes>.
    #[error(transparent)]
    SpecError(#[from] SpecError),

    /// Indicates that the supervisor was unable to initialise due to an error.
    #[error("unable to initialize the supervisor: {0}")]
    Initialise(String),

    /// Indicates that error occurred while validating interop config.
    #[error(transparent)]
    InteropValidationError(#[from] ConfigError),

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

/// Extending the [`SuperchainDAError`] to include errors not in the spec.
#[derive(Error, Debug, PartialEq, Eq, derive_more::TryFrom)]
#[repr(i32)]
#[try_from(repr)]
pub enum SpecError {
    /// [`SuperchainDAError`] from the spec.
    #[error(transparent)]
    SuperchainDAError(#[from] SuperchainDAError),

    /// Error not in spec.
    #[error("error not in spec")]
    ErrorNotInSpec,
}

impl SpecError {
    /// Maps the proper error code from SuperchainDAError.
    /// Introduced a new error code for errors not in the spec.
    pub const fn code(&self) -> i32 {
        match self {
            Self::SuperchainDAError(e) => *e as i32,
            Self::ErrorNotInSpec => -321300,
        }
    }
}

impl From<SpecError> for ErrorObjectOwned {
    fn from(err: SpecError) -> Self {
        ErrorObjectOwned::owned(err.code(), err.to_string(), None::<()>)
    }
}

impl From<SupervisorError> for ErrorObjectOwned {
    fn from(err: SupervisorError) -> Self {
        match err {
            // todo: handle these errors more gracefully
            SupervisorError::Unimplemented |
            SupervisorError::EmptyDependencySet |
            SupervisorError::InteropNotEnabled |
            SupervisorError::L1BlockMismatch { .. } |
            SupervisorError::Initialise(_) |
            SupervisorError::ManagedNodeError(_) |
            SupervisorError::ChainProcessorError(_) |
            SupervisorError::CrossSafetyCheckerError(_) |
            SupervisorError::StorageError(_) |
            SupervisorError::InteropValidationError(_) |
            SupervisorError::AccessListError(_) => ErrorObjectOwned::from(ErrorCode::InternalError),
            SupervisorError::SpecError(err) => err.into(),
        }
    }
}

impl From<StorageError> for SpecError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::Database(_) => Self::from(SuperchainDAError::DataCorruption),
            StorageError::FutureData => Self::from(SuperchainDAError::FutureData),
            StorageError::EntryNotFound(_) => Self::from(SuperchainDAError::MissedData),
            StorageError::DatabaseNotInitialised => {
                Self::from(SuperchainDAError::UninitializedChainDatabase)
            }
            StorageError::ConflictError => Self::from(SuperchainDAError::ConflictingData),
            StorageError::BlockOutOfOrder => Self::from(SuperchainDAError::OutOfOrder),
            _ => Self::ErrorNotInSpec,
        }
    }
}

#[cfg(test)]
mod test {
    use kona_supervisor_storage::EntryNotFoundError;

    use super::*;

    #[test]
    fn test_storage_error_conversion() {
        let test_err = SpecError::from(StorageError::DatabaseNotInitialised);
        let expected_err =
            SpecError::SuperchainDAError(SuperchainDAError::UninitializedChainDatabase);

        assert_eq!(test_err, expected_err);
    }

    #[test]
    fn test_unmapped_storage_error_conversion() {
        let spec_err = ErrorObjectOwned::from(SpecError::ErrorNotInSpec);
        let expected_err = SpecError::ErrorNotInSpec;

        assert_eq!(spec_err, expected_err.into());

        let spec_err = ErrorObjectOwned::from(SpecError::from(StorageError::LockPoisoned));
        let expected_err = SpecError::ErrorNotInSpec;

        assert_eq!(spec_err, expected_err.into());

        let spec_err = ErrorObjectOwned::from(SpecError::from(StorageError::FutureData));
        let expected_err = SpecError::SuperchainDAError(SuperchainDAError::FutureData);

        assert_eq!(spec_err, expected_err.into());

        let spec_err = ErrorObjectOwned::from(SpecError::from(StorageError::EntryNotFound(
            EntryNotFoundError::DerivedBlockNotFound(12),
        )));
        let expected_err = SpecError::SuperchainDAError(SuperchainDAError::MissedData);

        assert_eq!(spec_err, expected_err.into());
    }

    #[test]
    fn test_supervisor_error_conversion() {
        // This will happen implicitly in server rpc response calls.
        let supervisor_err = ErrorObjectOwned::from(SupervisorError::SpecError(SpecError::from(
            StorageError::LockPoisoned,
        )));
        let expected_err = SpecError::ErrorNotInSpec;

        assert_eq!(supervisor_err, expected_err.into());

        let supervisor_err = ErrorObjectOwned::from(SupervisorError::SpecError(SpecError::from(
            StorageError::FutureData,
        )));
        let expected_err = SpecError::SuperchainDAError(SuperchainDAError::FutureData);

        assert_eq!(supervisor_err, expected_err.into());
    }
}
