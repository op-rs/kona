//! Main database access structure and transaction contexts.

use crate::{error::StorageError, providers::DerivationProvider, traits::DerivationStorage};
use alloy_eips::eip1898::BlockNumHash;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use reth_db::{
    DatabaseEnv,
    mdbx::{DatabaseArguments, init_db_for},
};
use reth_db_api::database::Database;
use std::path::Path;

/// Manages the database environment for a single chain.
/// Provides transactional access to data via providers.
#[derive(Debug)]
pub struct ChainDb {
    env: DatabaseEnv,
}

impl ChainDb {
    /// Creates or opens a database environment at the given path.
    pub fn new(path: &Path) -> Result<Self, StorageError> {
        let env = init_db_for::<_, crate::models::Tables>(path, DatabaseArguments::default())?;
        Ok(Self { env })
    }
}

impl DerivationStorage for ChainDb {
    fn derived_to_source(&self, derived_block_id: BlockNumHash) -> Result<BlockInfo, StorageError> {
        self.env.view(|tx| DerivationProvider::new(tx).derived_to_source(derived_block_id))?
    }

    fn latest_derived_block_at_source(
        &self,
        source_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError> {
        self.env.view(|tx| {
            DerivationProvider::new(tx).latest_derived_block_at_source(source_block_id)
        })?
    }

    fn latest_derived_block_pair(&self) -> Result<DerivedRefPair, StorageError> {
        self.env.view(|tx| DerivationProvider::new(tx).latest_derived_block_pair())?
    }

    fn save_derived_block_pair(&self, incoming_pair: DerivedRefPair) -> Result<(), StorageError> {
        self.env
            .update(|ctx| DerivationProvider::new(ctx).save_derived_block_pair(incoming_pair))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_open_db() {
        let tmp_dir = TempDir::new().expect("create temp dir");
        let db_path = tmp_dir.path().join("chaindb");
        let db = ChainDb::new(&db_path);
        assert!(db.is_ok(), "Should create or open database");
    }
}
