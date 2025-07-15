use super::{ManagedNodeClient, ManagedNodeError};
use alloy_eips::BlockNumHash;
use kona_supervisor_storage::{DerivationStorageReader, HeadRefStorageReader, StorageError};
use kona_supervisor_types::SuperHead;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Debug)]
pub(super) struct Resetter<DB, C> {
    client: Arc<C>,
    db_provider: Arc<DB>,
    reset_guard: Mutex<()>,
}

impl<DB, C> Resetter<DB, C>
where
    DB: HeadRefStorageReader + DerivationStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    /// Creates a new [`Resetter`] with the specified client.
    pub(super) fn new(client: Arc<C>, db_provider: Arc<DB>) -> Self {
        Self { client, db_provider, reset_guard: Mutex::new(()) }
    }

    /// Resets the node using the latest super head.
    pub(crate) async fn reset(&self) -> Result<(), ManagedNodeError> {
        let _guard = self.reset_guard.lock().await;

        info!(target: "resetter", "Resetting the node");

        let SuperHead { local_unsafe, cross_unsafe, local_safe, cross_safe, finalized, .. } =
            match self.get_latest_valid_super_head().await {
                Ok(block) => block,
                // todo: require refactor and corner case handling
                Err(ManagedNodeError::StorageError(StorageError::DatabaseNotInitialised)) => {
                    self.reset_pre_interop().await?;
                    return Ok(());
                }
                Err(err) => {
                    error!(target: "resetter", %err, "Failed to get latest valid derived block");
                    return Err(ManagedNodeError::ResetFailed);
                }
            };

        info!(target: "resetter",
            %local_unsafe,
            %cross_unsafe,
            %local_safe,
            %cross_safe,
            %finalized,
            "Resetting managed node with latest information",
        );

        self.client
            .reset(
                local_unsafe.id(),
                cross_unsafe.id(),
                local_safe.id(),
                cross_safe.id(),
                finalized.id(),
            )
            .await
            .inspect_err(|err| {
                error!(target: "resetter", %err, "Failed to reset managed node");
            })?;
        Ok(())
    }

    async fn reset_pre_interop(&self) -> Result<(), ManagedNodeError> {
        info!(target: "resetter", "Resetting the node to pre-interop state");

        self.client.reset_pre_interop().await.inspect_err(|err| {
            error!(target: "resetter", %err, "Failed to reset managed node to pre-interop state");
        })?;
        Ok(())
    }

    /// Gets the last valid derived block by walking back the source blocks and checking the hash of
    /// the last derived block at the source block.
    async fn get_latest_valid_super_head(&self) -> Result<SuperHead, ManagedNodeError> {
        // ToDo: right now the assumption is that supervisor has the correct view of the canonical
        // chain
        let mut super_head = self
            .db_provider
            .get_super_head()
            .inspect_err(|err| error!(target: "resetter", %err, "Failed to get super head"))?;

        let mut local_safe = super_head.local_safe;

        loop {
            let node_block = self.client.block_ref_by_number(local_safe.number).await.inspect_err(
                |err| error!(target: "resetter", %err, "Failed to get block by number"),
            )?;

            // If the local safe block matches the node block, we can return the super
            // head right away
            if node_block == local_safe {
                super_head.local_unsafe = local_safe;
                super_head.local_safe = local_safe;

                for ref_block in [
                    &mut super_head.finalized,
                    &mut super_head.cross_safe,
                    &mut super_head.cross_unsafe,
                ] {
                    if ref_block.number > local_safe.number {
                        *ref_block = local_safe;
                    }
                }
                return Ok(super_head);
            }

            // Get the source block for the current local safe, this helps to skip empty source
            // blocks
            let source_block = self
                .db_provider
                .derived_to_source(local_safe.id())
                .inspect_err(|err| error!(target: "resetter", %err, "Failed to get source block for the local safe head ref"))?;

            // Get the previous source block id
            let prev_source_id =
                BlockNumHash { number: source_block.number - 1, hash: source_block.parent_hash };

            // If the previous source block id is 0, we cannot reset further. This should not happen
            // in prod, added for safety during dev environment.
            if prev_source_id.number == 0 {
                error!(target: "resetter", "Source block number is 0, cannot reset further");
                return Err(ManagedNodeError::ResetFailed);
            }

            // Get the latest derived block at the previous source block, this helps to skip derived
            // blocks. If this loop is executed, it means there is something wrong with
            // derivation. Faster to go back source blocks than to go back derived
            // blocks.
            local_safe = self
                .db_provider
                .latest_derived_block_at_source(prev_source_id)
                .inspect_err(|err| {
                    error!(target: "resetter", %err, "Failed to get latest derived block for the previous source block")
                })?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncnode::{AuthenticationError, ClientError};
    use alloy_eips::BlockNumHash;
    use alloy_primitives::{B256, ChainId};
    use async_trait::async_trait;
    use jsonrpsee::core::client::Subscription;
    use kona_interop::{DerivedRefPair, SafetyLevel};
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{DerivationStorageReader, HeadRefStorageReader, StorageError};
    use kona_supervisor_types::{OutputV0, Receipts, SubscriptionEvent, SuperHead};
    use mockall::{mock, predicate};

    // Mock for HeadRefStorageReader
    mock! {
        #[derive(Debug)]
        pub Db {}

        impl HeadRefStorageReader for Db {
            fn get_safety_head_ref(&self, level: SafetyLevel) -> Result<BlockInfo, StorageError>;
            fn get_super_head(&self) -> Result<SuperHead, StorageError>;
        }

        impl DerivationStorageReader for Db {
            fn derived_to_source(&self, derived_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derived_block_at_source(&self, source_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derivation_state(&self) -> Result<DerivedRefPair, StorageError>;
        }
    }

    mock! {
        #[derive(Debug)]
        pub Client {}

        #[async_trait]
        impl ManagedNodeClient for Client {
            async fn chain_id(&self) -> Result<ChainId, ClientError>;
            async fn subscribe_events(&self) -> Result<Subscription<SubscriptionEvent>, ClientError>;
            async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ClientError>;
            async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ClientError>;
            async fn pending_output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ClientError>;
            async fn l2_block_ref_by_timestamp(&self, timestamp: u64) -> Result<BlockInfo, ClientError>;
            async fn block_ref_by_number(&self, block_number: u64) -> Result<BlockInfo, ClientError>;
            async fn reset_pre_interop(&self) -> Result<(), ClientError>;
            async fn reset(&self, unsafe_id: BlockNumHash, cross_unsafe_id: BlockNumHash, local_safe_id: BlockNumHash, cross_safe_id: BlockNumHash, finalised_id: BlockNumHash) -> Result<(), ClientError>;
            async fn provide_l1(&self, block_info: BlockInfo) -> Result<(), ClientError>;
            async fn update_finalized(&self, finalized_block_id: BlockNumHash) -> Result<(), ClientError>;
            async fn update_cross_unsafe(&self, cross_unsafe_block_id: BlockNumHash) -> Result<(), ClientError>;
            async fn update_cross_safe(&self, source_block_id: BlockNumHash, derived_block_id: BlockNumHash) -> Result<(), ClientError>;
            async fn reset_ws_client(&self);
        }
    }

    fn make_super_head() -> SuperHead {
        SuperHead {
            local_unsafe: BlockInfo::new(B256::from([0u8; 32]), 5, B256::ZERO, 0),
            cross_unsafe: BlockInfo::new(B256::from([1u8; 32]), 4, B256::ZERO, 0),
            local_safe: BlockInfo::new(B256::from([2u8; 32]), 3, B256::ZERO, 0),
            cross_safe: BlockInfo::new(B256::from([3u8; 32]), 2, B256::ZERO, 0),
            finalized: BlockInfo::new(B256::from([4u8; 32]), 1, B256::ZERO, 0),
            l1_source: BlockInfo::new(B256::from([54u8; 32]), 100, B256::ZERO, 0),
        }
    }

    #[tokio::test]
    async fn test_reset_success() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_get_super_head().returning(move || Ok(super_head));

        let mut client = MockClient::new();
        client.expect_block_ref_by_number().returning(move |_| Ok(super_head.local_safe));

        client.expect_reset().returning(|_, _, _, _, _| Ok(()));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_ok());
    }

    #[tokio::test]
    async fn test_reset_db_error() {
        let mut db = MockDb::new();
        db.expect_get_super_head().returning(|| Err(StorageError::LockPoisoned));

        let client = MockClient::new();

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_err());
    }

    #[tokio::test]
    async fn test_reset_block_error() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_get_super_head().returning(move || Ok(super_head));

        let mut client = MockClient::new();
        client
            .expect_block_ref_by_number()
            .returning(|_| Err(ClientError::Authentication(AuthenticationError::InvalidHeader)));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_err());
    }

    #[tokio::test]
    async fn test_reset_inconsistency() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_get_super_head().returning(move || Ok(super_head));

        let prev_source_block = BlockInfo::new(B256::from([8u8; 32]), 101, B256::ZERO, 0);
        let current_source_block =
            BlockInfo::new(B256::from([7u8; 32]), 102, prev_source_block.hash, 0);
        let last_valid_derived_block = BlockInfo::new(B256::from([6u8; 32]), 9, B256::ZERO, 0);

        // return expected values when get_last_valid_derived_block() is called
        db.expect_derived_to_source()
            .with(predicate::eq(super_head.local_safe.id()))
            .returning(move |_| Ok(current_source_block));
        db.expect_latest_derived_block_at_source()
            .with(predicate::eq(prev_source_block.id()))
            .returning(move |_| Ok(last_valid_derived_block));

        // let prev_source_block = BlockInfo::new(B256::from([8u8; 32]), 101, B256::ZERO, 0);
        // let current_source_block =
        //     BlockInfo::new(B256::from([7u8; 32]), 102, prev_source_block.hash, 0);
        // let last_valid_derived_block = BlockInfo::new(B256::from([6u8; 32]), 9, B256::ZERO, 0);

        // // return expected values when get_last_valid_derived_block() is called
        // db.expect_derived_to_source()
        //     .with(predicate::eq(super_head.local_safe.id()))
        //     .returning(move |_| Ok(current_source_block));
        // db.expect_latest_derived_block_at_source()
        //     .with(predicate::eq(prev_source_block.id()))
        //     .returning(move |_| Ok(last_valid_derived_block));

        let mut client = MockClient::new();
        // Return a block that does not match local_safe
        client
            .expect_block_ref_by_number()
            .with(predicate::eq(super_head.local_safe.number))
            .returning(|_| Ok(BlockInfo::new(B256::from([4u8; 32]), 3, B256::ZERO, 0)));
        // On second call, return the last valid derived block
        client
            .expect_block_ref_by_number()
            .with(predicate::eq(last_valid_derived_block.number))
            .returning(move |_| Ok(last_valid_derived_block));
        client.expect_reset().times(1).returning(|_, _, _, _, _| Ok(()));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_ok());
    }

    #[tokio::test]
    async fn test_reset_rpc_error() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_get_super_head().returning(move || Ok(super_head));

        let mut client = MockClient::new();
        client.expect_block_ref_by_number().returning(move |_| Ok(super_head.local_safe));
        client.expect_reset().returning(|_, _, _, _, _| {
            Err(ClientError::Authentication(AuthenticationError::InvalidJwt))
        });

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        assert!(resetter.reset().await.is_err());
    }
}
