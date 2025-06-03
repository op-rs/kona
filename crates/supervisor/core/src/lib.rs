//! This crate contains the core logic for the Optimism Supervisor component.

/// Contains the main Supervisor struct and its implementation.
mod supervisor;
pub use supervisor::{Supervisor, SupervisorError, SupervisorService};

mod logindexer;
pub use logindexer::{
    LogIndexer, LogIndexerError, log_to_log_hash, log_to_message_payload, payload_hash_to_log_hash,
};
mod rpc;

pub use rpc::SupervisorRpc;

pub mod syncnode;

pub mod chain_processor;
