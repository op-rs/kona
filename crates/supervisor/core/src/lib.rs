//! This crate contains the core logic for the Optimism Supervisor component.

/// Contains the main Supervisor struct and its implementation.
mod supervisor;
pub use supervisor::{Supervisor, SupervisorError, SupervisorService};

mod rpc;
pub use rpc::SupervisorRpc;

/// Contains the managed node logic, which handles interactions with the op-node.
pub mod syncnode;

/// Contains the chain processor logic, which handles events processing from the managed node.
pub mod chain_processor;
