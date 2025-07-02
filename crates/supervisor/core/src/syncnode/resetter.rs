use super::ManagedNodeClient;
use kona_interop::SafetyLevel;
use kona_supervisor_storage::HeadRefStorageReader;
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
    DB: HeadRefStorageReader + Send + Sync + 'static,
    C: ManagedNodeClient + Send + Sync + 'static,
{
    /// Creates a new [`Resetter`] with the specified client.
    pub(super) fn new(client: Arc<C>, db_provider: Arc<DB>) -> Self {
        Self { client, db_provider, reset_guard: Mutex::new(()) }
    }

    /// Resets the node using the latest super head.
    pub(crate) async fn reset(&self) {
        let _guard = self.reset_guard.lock().await;

        info!(target: "resetter", "Resetting the node");

        let unsafe_ref = match self.db_provider.get_safety_head_ref(SafetyLevel::LocalUnsafe) {
            Ok(val) => val,
            Err(err) => {
                error!(target: "resetter", %err, "Failed to get unsafe head ref");
                return;
            }
        };

        let cross_unsafe_ref = match self.db_provider.get_safety_head_ref(SafetyLevel::CrossUnsafe)
        {
            Ok(val) => val,
            Err(err) => {
                error!(target: "resetter", %err, "Failed to get cross unsafe head ref");
                return;
            }
        };

        let local_safe_ref = match self.db_provider.get_safety_head_ref(SafetyLevel::LocalSafe) {
            Ok(val) => val,
            Err(err) => {
                error!(target: "resetter", %err, "Failed to get local safe head ref");
                return;
            }
        };

        let safe_ref = match self.db_provider.get_safety_head_ref(SafetyLevel::CrossSafe) {
            Ok(val) => val,
            Err(err) => {
                error!(target: "resetter", %err, "Failed to get safe head ref");
                return;
            }
        };

        let finalised_ref = match self.db_provider.get_safety_head_ref(SafetyLevel::Finalized) {
            Ok(val) => val,
            Err(err) => {
                error!(target: "resetter", %err, "Failed to get finalised head ref");
                return;
            }
        };

        let node_safe_ref = match self.client.block_ref_by_number(local_safe_ref.number).await {
            Ok(block) => block,
            Err(err) => {
                // todo: it's possible that supervisor is ahead of the op-node
                // in this case we should handle the error gracefully
                error!(target: "resetter", %err, "Failed to get block by number");
                return;
            }
        };

        // check with consistency with the op-node
        if node_safe_ref.hash != local_safe_ref.hash {
            // todo: handle this case
            error!(target: "resetter", "Local safe ref hash does not match node safe ref hash");
            return;
        }

        info!(target: "resetter",
            %unsafe_ref,
            %cross_unsafe_ref,
            %local_safe_ref,
            %safe_ref,
            %finalised_ref,
            "Resetting managed node with latest information",
        );

        if let Err(err) = self
            .client
            .reset(
                unsafe_ref.id(),
                cross_unsafe_ref.id(),
                local_safe_ref.id(),
                safe_ref.id(),
                finalised_ref.id(),
            )
            .await
        {
            error!(target: "resetter", %err, "Failed to reset managed node");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncnode::ManagedNodeError;
    use alloy_eips::BlockNumHash;
    use alloy_primitives::{B256, ChainId};
    use async_trait::async_trait;
    use jsonrpsee::core::client::Subscription;
    use kona_interop::SafetyLevel;
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{HeadRefStorageReader, StorageError};
    use kona_supervisor_types::{OutputV0, Receipts, SubscriptionEvent, SuperHead};
    use mockall::mock;

    // Mock for HeadRefStorageReader
    mock! {
        #[derive(Debug)]
        pub Db {}

        impl HeadRefStorageReader for Db {
            fn get_current_l1(&self) -> Result<BlockInfo, StorageError>;
            fn get_safety_head_ref(&self, level: SafetyLevel) -> Result<BlockInfo, StorageError>;
            fn get_super_head(&self) -> Result<SuperHead, StorageError>;
        }
    }

    mock! {
        #[derive(Debug)]
        pub Client {}

        #[async_trait]
        impl ManagedNodeClient for Client {
            async fn chain_id(&self) -> Result<ChainId, ManagedNodeError>;
            async fn subscribe_events(&self) -> Result<Subscription<SubscriptionEvent>, ManagedNodeError>;
            async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, ManagedNodeError>;
            async fn output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ManagedNodeError>;
            async fn pending_output_v0_at_timestamp(&self, timestamp: u64) -> Result<OutputV0, ManagedNodeError>;
            async fn l2_block_ref_by_timestamp(&self, timestamp: u64) -> Result<BlockInfo, ManagedNodeError>;
            async fn block_ref_by_number(&self, block_number: u64) -> Result<BlockInfo, ManagedNodeError>;
            async fn reset(&self, unsafe_id: BlockNumHash, cross_unsafe_id: BlockNumHash, local_safe_id: BlockNumHash, cross_safe_id: BlockNumHash, finalised_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn provide_l1(&self, block_info: BlockInfo) -> Result<(), ManagedNodeError>;
            async fn update_finalized(&self, finalized_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn update_cross_unsafe(&self, cross_unsafe_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn update_cross_safe(&self, source_block_id: BlockNumHash, derived_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
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
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::LocalUnsafe)
            .returning(move |_| Ok(super_head.local_unsafe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::CrossUnsafe)
            .returning(move |_| Ok(super_head.cross_unsafe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::LocalSafe)
            .returning(move |_| Ok(super_head.local_safe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::CrossSafe)
            .returning(move |_| Ok(super_head.cross_safe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::Finalized)
            .returning(move |_| Ok(super_head.finalized));

        let mut client = MockClient::new();
        client.expect_block_ref_by_number().returning(move |_| Ok(super_head.local_safe));

        client.expect_reset().returning(|_, _, _, _, _| Ok(()));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        resetter.reset().await;
        // You can assert logs or side effects if needed
    }

    #[tokio::test]
    async fn test_reset_db_error() {
        let mut db = MockDb::new();
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::LocalUnsafe)
            .returning(move |_| Err(StorageError::DatabaseNotInitialised));

        let client = MockClient::new();

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        resetter.reset().await;
        // Should return early, no panic
    }

    #[tokio::test]
    async fn test_reset_block_error() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::LocalUnsafe)
            .returning(move |_| Ok(super_head.local_unsafe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::CrossUnsafe)
            .returning(move |_| Ok(super_head.cross_unsafe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::LocalSafe)
            .returning(move |_| Ok(super_head.local_safe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::CrossSafe)
            .returning(move |_| Ok(super_head.cross_safe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::Finalized)
            .returning(move |_| Ok(super_head.finalized));

        let mut client = MockClient::new();
        client
            .expect_block_ref_by_number()
            .returning(|_| Err(ManagedNodeError::DatabaseNotInitialised));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        resetter.reset().await;
        // Should return early, no panic
    }

    #[tokio::test]
    async fn test_reset_consistency_error() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::LocalUnsafe)
            .returning(move |_| Ok(super_head.local_unsafe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::CrossUnsafe)
            .returning(move |_| Ok(super_head.cross_unsafe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::LocalSafe)
            .returning(move |_| Ok(super_head.local_safe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::CrossSafe)
            .returning(move |_| Ok(super_head.cross_safe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::Finalized)
            .returning(move |_| Ok(super_head.finalized));

        let mut client = MockClient::new();
        // Return a block that does not match local_safe
        client
            .expect_block_ref_by_number()
            .returning(|_| Ok(BlockInfo::new(B256::from([4u8; 32]), 3, B256::ZERO, 0)));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        resetter.reset().await;
        // Should return early, no panic
    }

    #[tokio::test]
    async fn test_reset_rpc_error() {
        let super_head = make_super_head();

        let mut db = MockDb::new();
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::LocalUnsafe)
            .returning(move |_| Ok(super_head.local_unsafe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::CrossUnsafe)
            .returning(move |_| Ok(super_head.cross_unsafe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::LocalSafe)
            .returning(move |_| Ok(super_head.local_safe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::CrossSafe)
            .returning(move |_| Ok(super_head.cross_safe));
        db.expect_get_safety_head_ref()
            .withf(|level| *level == SafetyLevel::Finalized)
            .returning(move |_| Ok(super_head.finalized));

        let mut client = MockClient::new();
        client.expect_block_ref_by_number().returning(move |_| Ok(super_head.local_safe));
        client
            .expect_reset()
            .returning(|_, _, _, _, _| Err(ManagedNodeError::DatabaseNotInitialised));

        let resetter = Resetter::new(Arc::new(client), Arc::new(db));

        resetter.reset().await;
        // Should log error, no panic
    }
}
