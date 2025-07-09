//! Provider for tracking block safety head reference
use crate::{StorageError, models::SafetyHeadRefs};
use kona_protocol::BlockInfo;
use op_alloy_consensus::interop::SafetyLevel;
use reth_db_api::transaction::{DbTx, DbTxMut};
use tracing::{error, warn};

/// A Safety Head Reference storage that wraps transactional reference.
pub(crate) struct SafetyHeadRefProvider<'tx, TX> {
    tx: &'tx TX,
}

impl<'tx, TX> SafetyHeadRefProvider<'tx, TX> {
    pub(crate) const fn new(tx: &'tx TX) -> Self {
        Self { tx }
    }
}

impl<TX> SafetyHeadRefProvider<'_, TX>
where
    TX: DbTx,
{
    pub(crate) fn get_safety_head_ref(
        &self,
        safety_level: SafetyLevel,
    ) -> Result<BlockInfo, StorageError> {
        let head_ref_key = safety_level.into();
        let result = self.tx.get::<SafetyHeadRefs>(head_ref_key).inspect_err(|err| {
            error!(
                target: "supervisor_storage",
                %safety_level,
                %err,
                "Failed to seek head reference"
            );
        })?;
        let block_ref = result.ok_or_else(|| {
            warn!(target: "supervisor_storage", %safety_level, "No head reference found");
            StorageError::EntryNotFound("no head reference found".to_string())
        })?;
        Ok(block_ref.into())
    }
}

impl<Tx> SafetyHeadRefProvider<'_, Tx>
where
    Tx: DbTxMut + DbTx,
{
    pub(crate) fn initialise(&self, anchor: BlockInfo) -> Result<(), StorageError> {
        match self.get_safety_head_ref(SafetyLevel::LocalUnsafe) {
            Ok(_) => Ok(()), // if it is set already, skip.
            Err(StorageError::EntryNotFound(_)) => {
                self.update_safety_head_ref(SafetyLevel::LocalUnsafe, &anchor)?;
                self.update_safety_head_ref(SafetyLevel::CrossUnsafe, &anchor)?;
                self.update_safety_head_ref(SafetyLevel::LocalSafe, &anchor)?;
                self.update_safety_head_ref(SafetyLevel::CrossSafe, &anchor)
            }
            Err(err) => Err(err),
        }
    }
    pub(crate) fn update_safety_head_ref(
        &self,
        safety_level: SafetyLevel,
        block_info: &BlockInfo,
    ) -> Result<(), StorageError> {
        self.tx.put::<SafetyHeadRefs>(safety_level.into(), (*block_info).into()).inspect_err(
            |err| {
                error!(
                    target: "supervisor_storage",
                    %safety_level,
                    %err,
                    "Failed to store head reference"
                )
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Tables;
    use reth_db::{
        DatabaseEnv,
        mdbx::{DatabaseArguments, init_db_for},
    };
    use reth_db_api::Database;
    use tempfile::TempDir;

    fn setup_db() -> DatabaseEnv {
        let temp_dir = TempDir::new().expect("Could not create temp dir");
        init_db_for::<_, Tables>(temp_dir.path(), DatabaseArguments::default())
            .expect("Failed to init database")
    }

    #[test]
    fn test_safety_head_ref_retrieval() {
        let db = setup_db();

        // Create write transaction first
        let write_tx = db.tx_mut().expect("Failed to create write transaction");
        let write_provider = SafetyHeadRefProvider::new(&write_tx);

        // Initially, there should be no head ref
        let result = write_provider.get_safety_head_ref(SafetyLevel::CrossSafe);
        assert!(result.is_err());

        // Update head ref
        let block_info = BlockInfo::default();
        write_provider
            .update_safety_head_ref(SafetyLevel::CrossSafe, &block_info)
            .expect("Failed to update head ref");

        // Commit the write transaction
        write_tx.commit().expect("Failed to commit the write transaction");

        // Create a new read transaction to verify
        let tx = db.tx().expect("Failed to create transaction");
        let provider = SafetyHeadRefProvider::new(&tx);
        let result =
            provider.get_safety_head_ref(SafetyLevel::CrossSafe).expect("Failed to get head ref");
        assert_eq!(result, block_info);
    }

    #[test]
    fn test_safety_head_ref_update() {
        let db = setup_db();
        let write_tx = db.tx_mut().expect("Failed to create write transaction");
        let write_provider = SafetyHeadRefProvider::new(&write_tx);

        // Create initial block info
        let initial_block_info = BlockInfo {
            hash: Default::default(),
            number: 1,
            parent_hash: Default::default(),
            timestamp: 100,
        };
        write_provider
            .update_safety_head_ref(SafetyLevel::CrossSafe, &initial_block_info)
            .expect("Failed to update head ref");

        // Create updated block info
        let mut updated_block_info = BlockInfo {
            hash: Default::default(),
            number: 1,
            parent_hash: Default::default(),
            timestamp: 200,
        };
        updated_block_info.number = 100;
        write_provider
            .update_safety_head_ref(SafetyLevel::CrossSafe, &updated_block_info)
            .expect("Failed to update head ref");

        // Commit the write transaction
        write_tx.commit().expect("Failed to commit the write transaction");

        // Verify the updated value
        let tx = db.tx().expect("Failed to create transaction");
        let provider = SafetyHeadRefProvider::new(&tx);
        let result =
            provider.get_safety_head_ref(SafetyLevel::CrossSafe).expect("Failed to get head ref");
        assert_eq!(result, updated_block_info);
    }
}
