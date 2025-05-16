use core::fmt::Debug;

use alloy_primitives::B256;
use async_trait::async_trait;
use kona_interop::{DependencySet, ExecutingDescriptor, SafetyLevel};
use thiserror::Error;

/// Custom error type for the Supervisor core logic.
#[derive(Debug, Error)]
pub enum SupervisorError {
    /// Indicates that a feature or method is not yet implemented.
    #[error("functionality not implemented")]
    Unimplemented,
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
