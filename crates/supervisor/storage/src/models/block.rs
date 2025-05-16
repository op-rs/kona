//! Models for storing block metadata in the database.
//!
//! This module defines the data structure and schema used for tracking
//! individual blocks by block number. The stored metadata includes block hash,
//! parent hash, and block timestamp.
//!
//! Unlike logs, each block is uniquely identified by its number and does not
//! require dup-sorting.

use alloy_primitives::B256;
use reth_codecs::Compact;
use serde::{Deserialize, Serialize};

/// Metadata reference for a single block.
///
/// This struct captures minimal but essential block header information required
/// to track canonical block lineage and verify ancestry.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Compact)]
pub struct BlockHeader {
    /// The hash of the block itself.
    pub hash: B256,
    /// The hash of the parent block (previous block in the chain).
    pub parent_hash: B256,
    /// The timestamp of the block (seconds since Unix epoch).
    pub time: u64,
}
