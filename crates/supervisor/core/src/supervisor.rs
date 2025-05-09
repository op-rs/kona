use core::fmt::Debug;

use alloy_primitives::B256;
use kona_interop::{ExecutingDescriptor, SafetyLevel};
use async_trait::async_trait;
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
pub trait SupervisorService: Debug {
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
pub struct Supervisor {}

impl Supervisor {
  /// Creates a new [`Supervisor`] instance.
  pub fn new() -> Self {
    Self {}
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