use core::fmt::Debug;

use alloy_primitives::{B256, ChainId};
use async_trait::async_trait;
use jsonrpsee::types::{ErrorCode, ErrorObjectOwned};
use kona_interop::{DependencySet, ExecutingDescriptor, SafetyLevel};
use kona_supervisor_types::SuperHead;
use op_alloy_rpc_types::InvalidInboxEntry;
use thiserror::Error;

use crate::config::RollupConfigSet;

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
    InvalidInboxEntry(#[from] InvalidInboxEntry),
}

impl From<SupervisorError> for ErrorObjectOwned {
    fn from(err: SupervisorError) -> Self {
        match err {
            SupervisorError::Unimplemented | SupervisorError::EmptyDependencySet => {
                ErrorObjectOwned::from(ErrorCode::InternalError)
            }
            SupervisorError::InvalidInboxEntry(err) => ErrorObjectOwned::owned(
                (err as i64).try_into().expect("should fit i32"),
                err.to_string(),
                None::<()>,
            ),
        }
    }
}

/// Defines the service for the Supervisor core logic.
#[async_trait]
#[auto_impl::auto_impl(&, &mut, Arc, Box)]
pub trait SupervisorService: Debug + Send + Sync {
    /// Returns list of supervised [`ChainId`]s.
    fn chain_ids(&self) -> impl Iterator<Item = ChainId>;

    /// Returns [`SuperHead`] of given supervised chain.
    fn super_head(&self, chain: ChainId) -> Result<SuperHead, SupervisorError>;

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
    dependency_set: DependencySet,
    _rollup_config_set: RollupConfigSet,
}

impl Supervisor {
    /// Creates a new [`Supervisor`] instance.
    #[allow(clippy::new_without_default, clippy::missing_const_for_fn)]
    pub fn new(dependency_set: DependencySet, rollup_config_set: RollupConfigSet) -> Self {
        Self { dependency_set, _rollup_config_set: rollup_config_set }
    }
}

#[async_trait]
impl SupervisorService for Supervisor {
    fn chain_ids(&self) -> impl Iterator<Item = ChainId> {
        self.dependency_set.dependencies.keys().copied()
    }

    fn super_head(&self, _chain: ChainId) -> Result<SuperHead, SupervisorError> {
        todo!("implement call to ChainDbFactory")
    }

    async fn check_access_list(
        &self,
        _inbox_entries: Vec<B256>,
        _min_safety: SafetyLevel,
        _executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        Err(SupervisorError::Unimplemented)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_rpc_error_conversion() {
        let err = InvalidInboxEntry::UnknownChain;
        let rpc_err = ErrorObjectOwned::owned(err as i32, err.to_string(), None::<()>);

        assert_eq!(ErrorObjectOwned::from(SupervisorError::InvalidInboxEntry(err)), rpc_err);
    }
}
