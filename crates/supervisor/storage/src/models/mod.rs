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
pub use block::BlockHeader;

/// Implements [`reth_db_api::table::Compress`] and [`reth_db_api::table::Decompress`] traits for
/// types that implement [`reth_codecs::Compact`].
///
/// This macro defines how to serialize and deserialize a type into a compressed
/// byte format using Reth's compact codec system.
///
/// # Example
/// ```ignore
/// impl_compression_for_compact!(BlockHeader, LogEntry);
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
impl_compression_for_compact!(BlockHeader, LogEntry);

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
    /// - Value: [`BlockHeader`] — block metadata
    table BlockHeaders {
        type Key = u64;
        type Value = BlockHeader;
    }
}
