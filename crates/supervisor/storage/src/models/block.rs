//! Models for storing block metadata in the database.
//!
//! This module defines the data structure and schema used for tracking
//! individual blocks by block number. The stored metadata includes block hash,
//! parent hash, and block timestamp.
//!
//! Unlike logs, each block is uniquely identified by its number and does not
//! require dup-sorting.

use alloy_primitives::B256;
use serde::{Deserialize, Serialize};
use reth_codecs::Compact;
use reth_db::table::Table;

/// Metadata reference for a single block.
///
/// This struct captures minimal but essential block header information required
/// to track canonical block lineage and verify ancestry. It is stored as the value
/// in the [`BlockHeaders`] table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Compact)]
pub struct BlockHeader {
    /// The hash of the block itself.
    pub hash: B256,

    /// The hash of the parent block (previous block in the chain).
    pub parent_hash: B256,

    /// The timestamp of the block (seconds since Unix epoch).
    pub time: u64,
}

/// A table for storing block metadata by block number.
///
/// This is a standard table (not dup-sorted) where:
/// - **Key**: `u64` — block number
/// - **Value**: [`BlockHeader`] — block metadata
///
/// This layout allows efficient retrieval of block header info
/// for ancestry checks and parent lookups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct BlockHeaders;

impl Table for BlockHeaders {
    /// Table name used internally by the backend database.
    const NAME: &'static str = "block_headers";

    /// Indicates that this is a single-entry table (not dup-sorted).
    const DUPSORT: bool = false;

    /// Primary key: block number.
    type Key = u64;

    /// Stored value: block metadata.
    type Value = BlockHeader;
}
