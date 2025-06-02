//! This crate contains the core logic for the Optimism Supervisor component.

/// Contains the main Supervisor struct and its implementation.
mod supervisor;
pub use supervisor::{Supervisor, SupervisorError, SupervisorService};

mod rpc;
pub use rpc::SupervisorRpc;

pub mod syncnode;

pub mod chain_processor;
