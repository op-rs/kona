//! Database table schemas used by the Supervisor.
//!
//! This module defines the value types, keys, and table layouts for all data
//! persisted by the `supervisor` component of the node.
//!
//! The tables are registered using [`reth_db_api::table::TableInfo`] and grouped into a
//! [`reth_db_api::TableSet`] for database initialization via Reth's storage-api.

use reth_db_api::{
    TableSet, TableType, TableViewer,
    table::{DupSort, TableInfo},
    tables,
};
use std::fmt;

mod log;
pub use log::{ExecutingMessageEntry, LogEntry};
mod block;
pub use block::BlockRef;

/// Implements [`reth_db_api::table::Compress`] and [`reth_db_api::table::Decompress`] traits for
/// types that implement [`reth_codecs::Compact`].
///
/// This macro defines how to serialize and deserialize a type into a compressed
/// byte format using Reth's compact codec system.
///
/// # Example
/// ```ignore
/// impl_compression_for_compact!(BlockRef, LogEntry);
/// ```
macro_rules! impl_compression_for_compact {
    ($($name:ident$(<$($generic:ident),*>)?),+) => {
        $(
            impl$(<$($generic: core::fmt::Debug + Send + Sync + Compact),*>)? reth_db_api::table::Compress for $name$(<$($generic),*>)? {
                type Compressed = Vec<u8>;

                fn compress_to_buf<B: bytes::BufMut + AsMut<[u8]>>(&self, buf: &mut B) {
                    let _ = reth_codecs::Compact::to_compact(self, buf);
                }
            }

            impl$(<$($generic: core::fmt::Debug + Send + Sync + Compact),*>)? reth_db_api::table::Decompress for $name$(<$($generic),*>)? {
                fn decompress(value: &[u8]) -> Result<$name$(<$($generic),*>)?, reth_db_api::DatabaseError> {
                    let (obj, _) = reth_codecs::Compact::from_compact(value, value.len());
                    Ok(obj)
                }
            }
        )+
    };
}

// Implement compression logic for all value types stored in tables
impl_compression_for_compact!(BlockRef, LogEntry);

tables! {
    /// A dup-sorted table that stores all logs emitted in a given block, sorted by their index.
    /// Keyed by block number, with log index as the subkey for DupSort.
    table LogEntries {
        type Key = u64;       // Primary key: u64 (block_number)
        type Value = LogEntry; // Value: The log metadata
        type SubKey = u32;    // SubKey for DupSort: u32 (log_index)
    }

    /// A table for storing block metadata by block number.
    /// This is a standard table (not dup-sorted) where:
    /// - Key: `u64` — block number
    /// - Value: [`BlockRef`] — block metadata
    table BlockRefs {
        type Key = u64;
        type Value = BlockRef;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use reth_db_api::table::{Compress, Decompress};

    // Helper to create somewhat unique B256 values for testing.
    fn test_b256(val: u8) -> B256 {
        let mut val_bytes = [0u8; 32];
        val_bytes[0] = val; // Place the u8 into the first byte of the array
        let b256_from_val = B256::from(val_bytes);
        B256::random() ^ b256_from_val
    }

    #[test]
    fn test_block_ref_compression_decompression() {
        let original =
            BlockRef { number: 1, hash: test_b256(1), parent_hash: test_b256(2), time: 1234567890 };

        let mut compressed_buf = Vec::new();
        original.compress_to_buf(&mut compressed_buf);

        // Ensure some data was written
        assert!(!compressed_buf.is_empty());

        let decompressed = BlockRef::decompress(&compressed_buf).unwrap();
        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_log_entry_compression_decompression_with_message() {
        let original = LogEntry {
            hash: test_b256(3),
            executing_message: Some(ExecutingMessageEntry {
                chain_id: 1,
                block_number: 100,
                log_index: 2,
                timestamp: 12345,
                hash: test_b256(4),
            }),
        };

        let mut compressed_buf = Vec::new();
        original.compress_to_buf(&mut compressed_buf);
        assert!(!compressed_buf.is_empty());
        let decompressed = LogEntry::decompress(&compressed_buf).unwrap();
        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_log_entry_compression_decompression_without_message() {
        let original = LogEntry { hash: test_b256(5), executing_message: None };
        let mut compressed_buf = Vec::new();
        original.compress_to_buf(&mut compressed_buf);
        assert!(!compressed_buf.is_empty());
        let decompressed = LogEntry::decompress(&compressed_buf).unwrap();
        assert_eq!(original, decompressed);
    }
}
