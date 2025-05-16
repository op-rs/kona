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
