use core::fmt::Debug;

use alloy_primitives::B256;
use async_trait::async_trait;
use jsonrpsee::types::ErrorObjectOwned;
use kona_interop::{DependencySet, ExecutingDescriptor, SafetyLevel};
use kona_supervisor_rpc::SupervisorApiServer;
use op_alloy_rpc_types::InvalidInboxEntry;
use thiserror::Error;

/// Custom error type for the Supervisor core logic.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SupervisorError {
    /// Indicates that a feature or method is not yet implemented.
    #[error("functionality not implemented")]
    Unimplemented,
    /// Data availability errors.
    ///
    /// Spec <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#protocol-specific-error-codes>.
    #[error(transparent)]
    InvalidInboxEntry(#[from] InvalidInboxEntry),
}

impl From<ErrorObjectOwned> for SupervisorError {
    fn from(err: ErrorObjectOwned) -> Self {
        let Ok(err) = (err.code() as i64).try_into() else {
            return Self::Unimplemented;
        };
        Self::InvalidInboxEntry(err)
    }
}

/// Defines the service for the Supervisor core logic.
#[async_trait]
pub trait SupervisorService: Debug + Send + Sync {
    /// Verifies if an access-list references only valid messages
    async fn check_access_list(
        &self,
        _inbox_entries: Vec<B256>,
        _min_safety: SafetyLevel,
        _executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        Err(SupervisorError::Unimplemented)
    }
}

/// The core Supervisor component responsible for monitoring and coordinating chain states.
#[derive(Debug)]
pub struct Supervisor {
    _dependency_set: DependencySet,
}

impl Supervisor {
    /// Creates a new [`Supervisor`] instance.
    #[allow(clippy::new_without_default, clippy::missing_const_for_fn)]
    pub fn new(dependency_set: DependencySet) -> Self {
        Self { _dependency_set: dependency_set }
    }
}

#[async_trait]
impl SupervisorService for Supervisor {
    async fn check_access_list(
        &self,
        _inbox_entries: Vec<B256>,
        _min_safety: SafetyLevel,
        _executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        Err(SupervisorError::Unimplemented)
    }
}

#[async_trait]
impl<T> SupervisorService for T
where
    T: SupervisorApiServer + Debug,
{
    async fn check_access_list(
        &self,
        inbox_entries: Vec<B256>,
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        // codecov:ignore-start
        Ok(T::check_access_list(self, inbox_entries, min_safety, executing_descriptor).await?)
        // codecov:ignore-end
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_rpc_error_conversion() {
        let err_code = InvalidInboxEntry::UnknownChain;
        let err = ErrorObjectOwned::owned(err_code as i32, "", None::<()>);

        assert_eq!(SupervisorError::InvalidInboxEntry(err_code), err.into());

        let err = ErrorObjectOwned::owned(i32::MAX, "", None::<()>);

        assert_eq!(SupervisorError::Unimplemented, err.into())
    }
}
