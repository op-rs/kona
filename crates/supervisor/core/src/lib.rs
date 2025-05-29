//! This crate contains the core logic for the Optimism Supervisor component.

/// Contains the main Supervisor struct and its implementation.
mod supervisor;
pub use supervisor::{Supervisor, SupervisorError, SupervisorService};

mod rpc;
pub use rpc::SupervisorRpc;

mod syncnode;
pub use syncnode::{ManagedNode, ManagedNodeConfig, ManagedNodeError, NodeEvent};
