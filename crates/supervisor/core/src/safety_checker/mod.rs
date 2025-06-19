//! # Cross-Chain Block Safety Checker
//!
//! This module is responsible for verifying that all executing messages in a block
//! are based on dependencies that have reached the required safety level (e.g.,
//! [`CrossSafe`](op_alloy_consensus::interop::SafetyLevel)).
//!
//! It ensures correctness in cross-chain execution by validating that initiating blocks
//! of messages are safely committed before the messages are executed in other chains.
mod cross;
pub use cross::CrossSafetyChecker;
mod error;
mod task;
pub use task::CrossSafetyCheckerJob;

pub use error::CrossSafetyError;
