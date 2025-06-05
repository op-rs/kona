#![doc = include_str!("../README.md")]

#[cfg(feature = "jsonrpsee")]
pub mod jsonrpsee;
#[cfg(all(feature = "jsonrpsee", feature = "client"))]
pub use jsonrpsee::SupervisorApiClient;
#[cfg(feature = "jsonrpsee")]
pub use jsonrpsee::SupervisorApiServer;

#[cfg(all(feature = "jsonrpsee", feature = "client"))]
pub use jsonrpsee::ManagedModeApiClient;

pub mod response;
pub use response::{SupervisorChainSyncStatus, SupervisorSyncStatus};

pub use kona_protocol::BlockInfo;
