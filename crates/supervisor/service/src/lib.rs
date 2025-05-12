//! This crate provides the runnable service layer for the Kona Supervisor.
//! It integrates the core logic with the RPC server.

mod service;

pub use service::{Config, Service};

mod actors;
pub use actors::{L1WatcherRpc, L1WatcherRpcError, SupervisorActor};
