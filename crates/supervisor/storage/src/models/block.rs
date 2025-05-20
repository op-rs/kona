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
/// This struct captures essential block information required to track canonical
/// block lineage and verify ancestry. It is stored as the value
/// in the [`crate::models::BlockRefs`] table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Compact)]
pub struct BlockRef {
    /// The height of the block.
    pub number: u64,
    /// The hash of the block itself.
    pub hash: B256,
    /// The hash of the parent block (previous block in the chain).
    pub parent_hash: B256,
    /// The timestamp of the block (seconds since Unix epoch).
    pub time: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;

    fn test_b256(val: u8) -> B256 {
        let mut val_bytes = [0u8; 32];
        val_bytes[0] = val;
        let b256_from_val = B256::from(val_bytes);
        B256::random() ^ b256_from_val
    }

    #[test]
    fn test_block_ref_compact_roundtrip() {
        let original_ref = BlockRef {
            number: 42,
            hash: test_b256(10),
            parent_hash: test_b256(11),
            time: 1678886400,
        };

        let mut buffer = Vec::new();
        let bytes_written = original_ref.to_compact(&mut buffer);
        assert_eq!(bytes_written, buffer.len(), "Bytes written should match buffer length");

        let (deserialized_ref, remaining_buf) = BlockRef::from_compact(&buffer, bytes_written);
        assert_eq!(original_ref, deserialized_ref, "Original and deserialized ref should be equal");
        assert!(remaining_buf.is_empty(), "Remaining buffer should be empty after deserialization");
    }
}
