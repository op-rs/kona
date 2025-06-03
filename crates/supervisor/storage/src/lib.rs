//! Persistent storage for the Supervisor.
//!
//! This crate provides structured, append-only storage for the Supervisor,
//! exposing high-level APIs to write and query logs, block metadata, and
//! other execution states.
//!
//! The storage system is built on top of [`reth-db`], using MDBX,
//! and defines schemas for supervisor-specific data like:
//! - L2 log entries
//! - Block ancestry metadata
//! - Source and Derived Blocks
//! - Chain heads for safety levels: **SAFE**, **UNSAFE**, and **CROSS-SAFE**
//!
//!
//! ## Capabilities
//!
//! - Append logs emitted by L2 execution
//! - Look up logs by block number and index
//! - Rewind logs during reorgs
//! - Track sealed blocks and ancestry metadata

pub mod models;

mod error;
pub use error::StorageError;

mod providers;

mod chaindb;
pub use chaindb::ChainDb;

mod chaindb_factory;
pub use chaindb_factory::ChainDbFactory;

mod traits;
pub use traits::{
    DerivationStorage, LogStorage, LogStorageReader, LogStorageWriter, SafetyHeadRefStorage,
};
