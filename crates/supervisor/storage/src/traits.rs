use crate::StorageError;
use alloy_eips::eip1898::BlockNumHash;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;

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
