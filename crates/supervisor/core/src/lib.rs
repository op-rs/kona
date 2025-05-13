//! This crate contains the core logic for the Optimism Supervisor component.

/// Contains the main Supervisor struct and its implementation.
mod supervisor;
pub use supervisor::{Supervisor, SupervisorError, SupervisorService};

mod rpc;
mod logindexer;
pub use logindexer::LogIndexer;

pub use rpc::SupervisorRpc;
