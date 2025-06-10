use crate::StorageError;
use alloy_eips::eip1898::BlockNumHash;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_types::Log;
use op_alloy_consensus::interop::SafetyLevel;
use std::fmt::Debug;

/// Provides an interface for supervisor storage to manage source and derived blocks.
///
/// Defines methods to retrieve derived block information,
/// enabling the supervisor to track the derivation progress.
///
/// Implementations are expected to provide persistent and thread-safe access to block data.
pub trait DerivationStorageReader: Debug {
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
}

/// Provides an interface for supervisor storage to write source and derived blocks.
///
/// Defines methods to persist derived block information,
/// enabling the supervisor to track the derivation progress.
///
/// Implementations are expected to provide persistent and thread-safe access to block data.
pub trait DerivationStorageWriter: Debug {
    /// Saves a [`DerivedRefPair`] to the storage.
    /// This method is append only and does not overwrite existing pairs.
    /// Ensures that the latest stored pair is the parent of the incoming pair before saving.
    ///
    /// # Arguments
    /// * `incoming_pair` - The derived block pair to save.
    ///
    /// # Returns
    /// * `Ok(())` if the pair was successfully saved.
    /// * `Err(StorageError)` if there is an issue saving the pair.
    fn save_derived_block_pair(&self, incoming_pair: DerivedRefPair) -> Result<(), StorageError>;
}

/// Combines both reading and writing capabilities for derivation storage.
///
/// Any type that implements both [`DerivationStorageReader`] and [`DerivationStorageWriter`]
/// automatically implements this trait.
pub trait DerivationStorage: DerivationStorageReader + DerivationStorageWriter {}

impl<T: DerivationStorageReader + DerivationStorageWriter> DerivationStorage for T {}

/// Provides an interface for retrieving logs associated with blocks.
///
/// This trait defines methods to retrieve the latest block,
/// find a block by a specific log, and retrieve logs for a given block number.
///
/// Implementations are expected to provide persistent and thread-safe access to block logs.
pub trait LogStorageReader: Debug {
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
}

/// Provides an interface for storing blocks and  logs associated with blocks.
///
/// Implementations are expected to provide persistent and thread-safe access to block logs.
pub trait LogStorageWriter: Send + Sync + Debug {
    /// Stores [`BlockInfo`] and [`Log`]s in the storage.
    /// This method is append-only and does not overwrite existing logs.
    /// Ensures that the latest stored block is the parent of the incoming block before saving.
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

/// Combines both reading and writing capabilities for log storage.
///
/// Any type that implements both [`LogStorageReader`] and [`LogStorageWriter`]
/// automatically implements this trait.
pub trait LogStorage: LogStorageReader + LogStorageWriter {}

impl<T: LogStorageReader + LogStorageWriter> LogStorage for T {}

/// Provides an interface for retrieving safety head references.
///
/// This trait defines methods to manage safety head references for different safety levels.
/// Each safety level maintains a reference to a block.
///
/// Implementations are expected to provide persistent and thread-safe access to safety head
/// references.
pub trait SafetyHeadRefStorageReader: Debug {
    /// Retrieves the current [`BlockInfo`] for a given [`SafetyLevel`].
    ///
    /// # Arguments
    /// * `safety_level` - The safety level for which to retrieve the head reference.
    ///
    /// # Returns
    /// * `Ok(BlockInfo)` containing the current safety head reference.
    /// * `Err(StorageError)` if there is an issue retrieving the reference.
    fn get_safety_head_ref(&self, safety_level: SafetyLevel) -> Result<BlockInfo, StorageError>;
}

/// Provides an interface for storing safety head references.
///
/// This trait defines methods to manage safety head references for different safety levels.
/// Each safety level maintains a reference to a block.
///
/// Implementations are expected to provide persistent and thread-safe access to safety head
/// references.
pub trait SafetyHeadRefStorageWriter: Debug {
    /// Updates the safety head reference for a given [`SafetyLevel`].
    ///
    /// # Arguments
    /// * `safety_level` - The safety level for which to update the head reference.
    /// * `block` - The new [`BlockInfo`] to set as the safety head reference.
    ///
    /// # Returns
    /// * `Ok(())` if the reference was successfully updated.
    /// * `Err(StorageError)` if there is an issue updating the reference.
    fn update_safety_head_ref(
        &self,
        safety_level: SafetyLevel,
        block: &BlockInfo,
    ) -> Result<(), StorageError>;
}

/// Combines both reading and writing capabilities for safety head ref storage.
///
/// Any type that implements both [`SafetyHeadRefStorageReader`] and [`SafetyHeadRefStorageWriter`]
/// automatically implements this trait.
pub trait SafetyHeadRefStorage: SafetyHeadRefStorageReader + SafetyHeadRefStorageWriter {}

impl<T: SafetyHeadRefStorageReader + SafetyHeadRefStorageWriter> SafetyHeadRefStorage for T {}
