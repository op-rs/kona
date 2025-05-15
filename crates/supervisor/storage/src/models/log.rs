//! Models for storing blockchain logs in the database.
//!
//! This module defines the data structure and table mapping for logs emitted during
//! transaction execution. Each log is uniquely identified by its block number and
//! index within the block.
//!
//! The table is dup-sorted, allowing efficient grouping of multiple logs per block.
//! It supports fast appends, retrieval, and range queries ordered by log index.

use alloy_primitives::B256;
use reth_codecs::Compact;
use reth_db::table::Table;
use reth_db_api::table::DupSort;
use serde::{Deserialize, Serialize};

/// Metadata associated with a single emitted log.
///
/// This is the value stored in the `LogEntries` dup-sorted table. Each entry includes:
/// - `hash`: The keccak256 hash of the log event.
/// - `executing_message_hash`: Optional hash representing a message that executes this log (used in
///   cross-chain execution contexts).
/// - `timestamp`: Optional timestamp at when the executing message was created.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Compact)]
pub struct LogEntry {
    /// The keccak256 hash of the emitted log event.
    pub hash: B256,
    /// Optional cross-domain execution message hash.
    pub executing_message_hash: Option<B256>,
    /// Optional timestamp of the log (usually block timestamp).
    pub timestamp: Option<u64>,
}

/// A dup-sorted table that stores all logs emitted in a given block, sorted by their index.
///
/// ## Table Schema:
/// - **Key**: `u64` — Block number.
/// - **SubKey**: `u32` — Log index within the block.
/// - **Value**: [`LogEntry`] — Structured metadata about the log.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct LogEntries;

/// Implements the [`Table`] trait for [`LogEntries`] to define its physical storage layout.
///
/// This declares the primary key type (`u64`) and the value type [`LogEntry`],
/// and marks the table as dup-sorted so it supports multiple values per key.
impl Table for LogEntries {
    /// Internal name used in the database backend (e.g., MDBX).
    const NAME: &'static str = "log_entries";

    /// Indicates that this table is dup-sorted (multiple values per primary key).
    const DUPSORT: bool = true;

    /// Primary key: block number.
    type Key = u64;

    /// Value stored for each log.
    type Value = LogEntry;
}

/// Implements the [`DupSort`] trait for [`LogEntries`] to support ordered sub-keys.
///
/// This defines the secondary key (`u32`) used to sort logs by index within each block.
impl DupSort for LogEntries {
    /// SubKey: log index within the block.
    type SubKey = u32;
}
