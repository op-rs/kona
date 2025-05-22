//! Log storage abstraction for supervisor state tracking.
//!
//! This module defines traits for reading and writing logs and block metadata
//! associated with L2 execution. These traits are implemented by concrete
//! backends like [`mdbx`] and are used by the supervisor during block ingestion
//! and verification workflows.

mod mdbx;

use kona_protocol::BlockInfo;
use kona_supervisor_types::Log;

/// Provides read-only access to log and block metadata stored in the supervisor database.
///
/// This trait is used by components that need to inspect the state of execution logs
/// and block metadata without modifying storage.
#[allow(dead_code)]
pub(crate) trait LogStorageReader {
    /// The associated error type returned by read operations.
    type Error;

    /// Retrieves block metadata for a given block number.
    fn get_block(&self, block_number: u64) -> Result<BlockInfo, Self::Error>;

    /// Returns the most recent block stored in the database.
    fn get_latest_block(&self) -> Result<BlockInfo, Self::Error>;

    /// Retrieves the block that contains the specified log.
    ///
    /// # Arguments
    /// - `block_number`: The block in which the log is expected.
    /// - `log`: The full log object (used to match hash/index).
    fn get_block_by_log(&self, block_number: u64, log: &Log) -> Result<BlockInfo, Self::Error>;

    /// Retrieves all logs associated with the given block number.
    fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, Self::Error>;
}

/// Provides write access to supervisor log storage.
///
/// This trait is used by indexing and ingestion components to persist block-level
/// logs and associated block metadata. Writes are typically batched and scoped to
/// a transactional context to ensure consistency.
#[allow(dead_code)]
pub(crate) trait LogStorageWriter {
    /// The associated error type returned by write operations.
    type Error;

    /// Stores a full block of logs and associated block metadata.
    ///
    /// This method is typically called by the supervisor during
    /// ingestion of L2 execution results.
    ///
    /// # Arguments
    /// - `block`: The metadata for the block.
    /// - `logs`: The logs emitted within that block.
    fn store_block_logs(&self, block: &BlockInfo, logs: Vec<Log>) -> Result<(), Self::Error>;
}

/// Composite trait for log read/write operations.
///
/// This trait combines [`LogStorageReader`] and [`LogStorageWriter`] and can be used where full
/// access to log storage is required.
#[allow(dead_code)]
pub(crate) trait LogStorage: LogStorageReader + LogStorageWriter {}
