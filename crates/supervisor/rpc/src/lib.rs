#![doc = include_str!("../README.md")]

#[cfg(feature = "jsonrpsee")]
mod jsonrpsee;
#[cfg(all(feature = "jsonrpsee", feature = "client"))]
pub use jsonrpsee::SupervisorApiClient;
#[cfg(feature = "jsonrpsee")]
pub use jsonrpsee::SupervisorApiServer;
