use op_alloy_consensus::interop::SafetyLevel;
use reth_db_api::transaction::DbTx;
use tracing::{error, warn};
use kona_protocol::BlockInfo;
use crate::models::{HeadRefs};
use crate::StorageError;

pub(crate) struct  SafetyHeadRefProvider<'tx, TX> {
    tx: &'tx TX,
}

impl<'tx, TX> SafetyHeadRefProvider<'tx, TX> {
    #[expect(dead_code)]
    pub(crate) const fn new(tx: &'tx TX) -> Self {
        Self { tx }
    }
}

impl<TX> SafetyHeadRefProvider<'_, TX>
where
    TX: DbTx,
{
    pub fn get_safety_head_ref(&self, safety_level: SafetyLevel) -> Result<BlockInfo, StorageError> {
        let head_ref_key = safety_level.into();
        let result = self.tx.get::<HeadRefs>(head_ref_key).inspect_err(|error|{
            error!(
                target: "supervisor_storage",
                %safety_level,
                ?error,
                "Failed to seek head reference"
            );
        })?;
        let block_ref = result.ok_or_else(||{
            warn!(target: "supervisor_storage", %safety_level, "No head reference found");
            StorageError::EntryNotFound("No head reference found".to_string())
        })?;
        Ok(block_ref.into())
    }
}