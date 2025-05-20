//! Models for storing blockchain derivation in the database.
//!
//! This module defines the data structure and schema used for tracking
//! how blocks are derived from source. This is particularly relevant
//! in rollup contexts, such as linking an L2 block to its originating L1 block.

use super::BlockRef;
use reth_codecs::Compact;
use serde::{Deserialize, Serialize};

/// Represents a pair of blocks where one block [`derived`](`Self::derived`) is derived
/// from another [`source`](`Self::source`).
///
/// This structure is used to track the lineage of blocks where L2 blocks are derived from L1
/// blocks. It stores the [`BlockRef`] information for both the source and the derived blocks.
/// It is stored as value in the [`DerivedBlocks`](`crate::models::DerivedBlocks`) table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DerivedBlockPair {
    /// The block that was derived from the [`source`](`Self::source`) block.
    pub derived: BlockRef,
    /// The source block from which the [`derived`](`Self::derived`) block was created.
    pub source: BlockRef,
}

impl Compact for DerivedBlockPair {
    fn to_compact<B: bytes::BufMut + AsMut<[u8]>>(&self, buf: &mut B) -> usize {
        let mut bytes_written = 0;
        bytes_written += self.derived.to_compact(buf);
        bytes_written += self.source.to_compact(buf);
        bytes_written
    }

    fn from_compact(buf: &[u8], _len: usize) -> (Self, &[u8]) {
        let (derived, remaining_buf) = BlockRef::from_compact(buf, buf.len());
        let (source, final_remaining_buf) =
            BlockRef::from_compact(remaining_buf, remaining_buf.len());
        (Self { derived, source }, final_remaining_buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BlockRef;
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
        let source_ref =
            BlockRef { number: 100, hash: test_b256(1), parent_hash: test_b256(2), time: 1000 };
        let derived_ref =
            BlockRef { number: 200, hash: test_b256(3), parent_hash: test_b256(4), time: 1010 };

        let original_pair = DerivedBlockPair { source: source_ref, derived: derived_ref };

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
