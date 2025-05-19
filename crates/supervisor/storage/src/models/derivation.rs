//! Models for storing blockchain derivation in the database.
//!
//! This module defines the data structure and schema used for tracking
//! how blocks are derived from source. This is particularly relevant
//! in rollup contexts, such as linking an L2 block to its originating L1 block.

use super::BlockHeader;
use reth_codecs::Compact;
use serde::{Deserialize, Serialize};

/// Represents a pair of block where one block (derived) is derived from another (source).
///
/// This structure is used to track the lineage of blocks where L2 blocks are derived from L1
/// blocks. It stores the header information for both the source and the derived block. It is stored
/// as value in the [`crate::models::DerivedBlocks`] table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DerivedBlockPair {
    /// The block that was derived from the `source` block.
    pub derived: BlockHeader,
    /// The source block from which the `derived` block was created.
    pub source: BlockHeader,
}

// Manually implement Compact for DerivedBlockPair
impl Compact for DerivedBlockPair {
    fn to_compact<B: bytes::BufMut + AsMut<[u8]>>(&self, buf: &mut B) -> usize {
        let mut bytes_written = 0;
        // Encode the 'derived' BlockHeader first
        bytes_written += self.derived.to_compact(buf);
        // Then encode the 'source' BlockHeader
        bytes_written += self.source.to_compact(buf);
        bytes_written
    }

    // The `len` parameter here is the length of the buffer `buf` that pertains to this specific
    // DerivedBlockPair. Since BlockHeader::from_compact returns the remaining buffer, we don't
    // strictly need `len` if BlockHeader consumes exactly what it needs and returns the rest.
    fn from_compact(buf: &[u8], _len: usize) -> (Self, &[u8]) {
        let (derived, remaining_buf) = BlockHeader::from_compact(buf, buf.len());
        let (source, final_remaining_buf) =
            BlockHeader::from_compact(remaining_buf, remaining_buf.len());
        (Self { derived, source }, final_remaining_buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BlockHeader;
    use alloy_primitives::B256;
    use reth_codecs::Compact;

    fn test_b256(val: u8) -> B256 {
        let mut val_bytes = [0u8; 32];
        val_bytes[0] = val;
        let b256_from_val = B256::from(val_bytes);
        B256::random() ^ b256_from_val
    }

    #[test]
    fn test_derived_block_pair_compact_roundtrip() {
        let source_header =
            BlockHeader { number: 100, hash: test_b256(1), parent_hash: test_b256(2), time: 1000 };
        let derived_header =
            BlockHeader { number: 200, hash: test_b256(3), parent_hash: test_b256(4), time: 1010 };

        let original_pair = DerivedBlockPair { source: source_header, derived: derived_header };

        let mut buffer = Vec::new();
        let bytes_written = original_pair.to_compact(&mut buffer);

        assert_eq!(bytes_written, buffer.len(), "Bytes written should match buffer length");
        let (deserialized_pair, remaining_buf) =
            DerivedBlockPair::from_compact(&buffer, bytes_written);

        assert_eq!(
            original_pair, deserialized_pair,
            "Original and deserialized pairs should be equal"
        );
        assert!(remaining_buf.is_empty(), "Remaining buffer should be empty after deserialization");
    }
}
