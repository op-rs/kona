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
/// This is the value stored in the [`crate::models::LogEntries`] dup-sorted table. Each entry
/// includes:
/// - `hash`: The keccak256 hash of the log event.
/// - `executing_message` - An optional field that may contain a cross-domain execution message.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Compact)]
pub struct LogEntry {
    /// The keccak256 hash of the emitted log event.
    pub hash: B256,
    /// Optional cross-domain execution message.
    pub executing_message: Option<ExecutingMessageEntry>,
}

/// Represents an entry of an executing message, containing metadata
/// about the message's origin and context within the blockchain.
/// - `chain_id` (`u64`): The unique identifier of the blockchain where the message originated.
/// - `block_number` (`u64`): The block number in the blockchain where the message originated.
/// - `log_index` (`u64`): The index of the log entry within the block where the message was logged.
/// - `timestamp` (`u64`): The timestamp associated with the block where the message was recorded.
/// - `hash` (`B256`): The unique hash identifier of the message.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Compact)]
pub struct ExecutingMessageEntry {
    /// ID of the chain where the message was emitted.
    pub chain_id: u64,
    /// Block number in the source chain.
    pub block_number: u64,
    /// Log index within the block.
    pub log_index: u64,
    /// Timestamp of the block.
    pub timestamp: u64,
    /// Hash of the message.
    pub hash: B256,
}

#[cfg(test)]
mod tests {
    use super::*; // Imports LogEntry, ExecutingMessageEntry
    use alloy_primitives::B256;
    use reth_codecs::Compact; // For the Compact trait methods

    // Helper to create somewhat unique B256 values for testing.
    // Assumes the "rand" feature for alloy-primitives is enabled for tests.
    fn test_b256(val: u8) -> B256 {
        let mut val_bytes = [0u8; 32];
        val_bytes[0] = val;
        let b256_from_val = B256::from(val_bytes);
        B256::random() ^ b256_from_val
    }

    #[test]
    fn test_log_entry_compact_roundtrip_with_message() {
        let original_log_entry = LogEntry {
            hash: test_b256(1),
            executing_message: Some(ExecutingMessageEntry {
                chain_id: 10,
                block_number: 1001,
                log_index: 5,
                timestamp: 1234567890,
                hash: test_b256(2),
            }),
        };

        let mut buffer = Vec::new();
        let bytes_written = original_log_entry.to_compact(&mut buffer);

        assert_eq!(bytes_written, buffer.len(), "Bytes written should match buffer length");
        assert!(!buffer.is_empty(), "Buffer should not be empty after compression");

        let (deserialized_log_entry, remaining_buf) =
            LogEntry::from_compact(&buffer, bytes_written);

        assert_eq!(
            original_log_entry, deserialized_log_entry,
            "Original and deserialized log entries should be equal"
        );
        assert!(remaining_buf.is_empty(), "Remaining buffer should be empty after deserialization");
    }

    #[test]
    fn test_log_entry_compact_roundtrip_without_message() {
        let original_log_entry = LogEntry { hash: test_b256(3), executing_message: None };

        let mut buffer = Vec::new();
        let bytes_written = original_log_entry.to_compact(&mut buffer);

        assert_eq!(bytes_written, buffer.len(), "Bytes written should match buffer length");
        assert!(!buffer.is_empty(), "Buffer should not be empty after compression");

        let (deserialized_log_entry, remaining_buf) =
            LogEntry::from_compact(&buffer, bytes_written);

        assert_eq!(
            original_log_entry, deserialized_log_entry,
            "Original and deserialized log entries should be equal"
        );
        assert!(remaining_buf.is_empty(), "Remaining buffer should be empty after deserialization");
    }
}
