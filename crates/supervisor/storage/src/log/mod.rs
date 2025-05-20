//! Log storage abstraction for supervisor state tracking.
//!
//! This module defines the [`LogStorage`] trait, which provides an interface for
//! storing and retrieving log-related data from execution blocks. It supports both
//! block-level and log-level queries and is implemented in the [`mdbx`] backend.
mod mdbx;
pub use mdbx::MdbxLogStorage;

use kona_protocol::BlockInfo;
use kona_supervisor_types::Log;

/// A trait for storing and querying L2 logs and block metadata.
///
/// This is the core abstraction for the supervisor's log storage backend.
/// It allows writing logs for each block and querying by block number and logs
pub trait LogStorage {
    /// The associated error type returned by this storage backend.
    type Error;

    // --- Write operations ---

    /// Stores a full block of logs and associated block metadata.
    ///
    /// This method is typically called by the supervisor during
    /// ingestion of L2 execution results.
    ///
    /// # Arguments
    /// - `block`: The metadata for the block.
    /// - `logs`: The logs emitted within that block.
    fn store_block_logs(&self, block: &BlockInfo, logs: Vec<Log>) -> Result<(), Self::Error>;

    // --- Read operations: block-related ---

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

    // --- Read operations: log-related ---

    /// Retrieves all logs associated with the given block number.
    fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, Self::Error>;
}