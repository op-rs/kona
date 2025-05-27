use crate::StorageError;
use alloy_eips::eip1898::BlockNumHash;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_types::Log;

/// Provides an interface for supervisor storage to manage source and derived blocks.
///
/// Defines methods to retrieve and persist derived block information,
/// enabling the supervisor to track the derivation progress.
///
/// Implementations are expected to provide persistent and thread-safe access to block data.
pub trait DerivationStorage {
    /// Gets the source [`BlockInfo`] for a given derived block [`BlockNumHash`].
    ///
    /// # Arguments
    /// * `derived_block_id` - The identifier (number and hash) of the derived (L2) block.
    ///
    /// # Returns
    /// * `Ok(BlockInfo)` containing the source block information if it exists.
    /// * `Err(StorageError)` if there is an issue retrieving the source block.
    fn derived_to_source(&self, derived_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;

    /// Gets the latest derived [`BlockInfo`] associated with the given source block
    /// [`BlockNumHash`].
    ///
    /// # Arguments
    /// * `source_block_id` - The identifier (number and hash) of the L1 source block.
    ///
    /// # Returns
    /// * `Ok(BlockInfo)` containing the latest derived block information if it exists.
    /// * `Err(StorageError)` if there is an issue retrieving the derived block.
    fn latest_derived_block_at_source(
        &self,
        source_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError>;

    /// Gets the latest [`DerivedRefPair`] from the storage.
    ///
    /// # Returns
    ///
    /// * `Ok(DerivedRefPair)` containing the latest derived block pair if it exists.
    /// * `Err(StorageError)` if there is an issue retrieving the pair.
    fn latest_derived_block_pair(&self) -> Result<DerivedRefPair, StorageError>;

    /// Saves a [`DerivedRefPair`] to the storage.
    ///
    /// # Arguments
    /// * `incoming_pair` - The derived block pair to save.
    ///
    /// # Returns
    /// * `Ok(())` if the pair was successfully saved.
    /// * `Err(StorageError)` if there is an issue saving the pair.
    fn save_derived_block_pair(&self, incoming_pair: DerivedRefPair) -> Result<(), StorageError>;
}

/// Provides an interface for storing and retrieving logs associated with blocks.
///
/// This trait defines methods to store logs for a specific block, retrieve the latest block,
/// find a block by a specific log, and retrieve logs for a given block number.
///
/// Implementations are expected to provide persistent and thread-safe access to block logs.
pub trait LogStorage {
    /// Retrieves the latest [`BlockInfo`] from the storage.
    ///
    /// # Returns
    /// * `Ok(BlockInfo)` containing the latest block information.
    /// * `Err(StorageError)` if there is an issue retrieving the latest block.
    fn get_latest_block(&self) -> Result<BlockInfo, StorageError>;

    /// Finds a [`BlockInfo`] by a specific [`Log`].
    /// This method searches for the block that contains the specified log at the given
    /// block number.
    ///
    /// # Arguments
    /// * `block_number` - The block number to search for the log.
    /// * `log` - The [`Log`] to find the associated block.
    ///
    /// # Returns
    /// * `Ok(BlockInfo)` containing the block information if the log is found.
    /// * `Err(StorageError)` if there is an issue retrieving the block or if the log is not found.
    // todo: refactor the arguments to use a more structured type
    fn get_block_by_log(&self, block_number: u64, log: &Log) -> Result<BlockInfo, StorageError>;

    /// Retrieves all [`Log`]s associated with a specific block number.
    ///
    /// # Arguments
    /// * `block_number` - The block number for which to retrieve logs.
    ///
    /// # Returns
    /// * `Ok(Vec<Log>)` containing the logs associated with the block number.
    /// * `Err(StorageError)` if there is an issue retrieving the logs or if no logs are found.
    fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, StorageError>;

    /// Stores [`BlockInfo`] and [`Log`]s in the storage.
    ///
    /// # Arguments
    /// * `block` - [`BlockInfo`] to associate with the logs.
    /// * `logs` - The [`Log`] events associated with the block.
    ///
    /// # Returns
    /// * `Ok(())` if the logs were successfully stored.
    /// * `Err(StorageError)` if there is an issue storing the logs.
    fn store_block_logs(&self, block: &BlockInfo, logs: Vec<Log>) -> Result<(), StorageError>;
}
