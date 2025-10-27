//! CallData Source

use crate::{ChainProvider, DataAvailabilityProvider, PipelineError, PipelineResult};
use alloc::{boxed::Box, collections::VecDeque};
use alloy_consensus::{Transaction, TxEnvelope, transaction::SignerRecoverable};
use alloy_primitives::{Address, Bytes, map::HashMap};
use async_trait::async_trait;
use kona_protocol::BlockInfo;

/// A data iterator that reads from calldata.
#[derive(Debug, Clone)]
pub struct CalldataSource<CP>
where
    CP: ChainProvider + Send,
{
    /// The chain provider to use for the calldata source.
    pub chain_provider: CP,
    /// The batch inbox address.
    pub batch_inbox_address: Address,
    /// Current calldata.
    pub calldata: VecDeque<Bytes>,
    /// Whether the calldata source is open.
    pub open: bool,
}

impl<CP: ChainProvider + Send> CalldataSource<CP> {
    /// Creates a new calldata source.
    pub const fn new(chain_provider: CP, batch_inbox_address: Address) -> Self {
        Self { chain_provider, batch_inbox_address, calldata: VecDeque::new(), open: false }
    }

    /// Loads the calldata into the source if it is not open.
    async fn load_calldata(
        &mut self,
        block_ref: &BlockInfo,
        batcher_address: Address,
    ) -> Result<(), CP::Error> {
        if self.open {
            return Ok(());
        }

        let (_, txs) =
            self.chain_provider.block_info_and_transactions_by_hash(block_ref.hash).await?;

        // Get transaction receipts to check for reverted transactions
        // This is optional - if receipts are not available (e.g., for EOA-based inboxes),
        // we'll include all transactions that match other criteria
        let tx_success_map = match self.chain_provider.receipts_by_hash(block_ref.hash).await {
            Ok(receipts) => {
                let mut map = HashMap::new();
                for (i, receipt) in receipts.iter().enumerate() {
                    if let Some(tx) = txs.get(i) {
                        let success = match receipt.status {
                            alloy_consensus::Eip658Value::Eip658(success) => success,
                            alloy_consensus::Eip658Value::PostState(_) => true, // Pre-EIP658 receipts are considered successful if they exist
                        };
                        map.insert(tx.tx_hash(), success);
                    }
                }
                Some(map)
            }
            Err(_) => None, // No receipts available, skip revert filtering
        };

        self.calldata = txs
            .iter()
            .filter_map(|tx| {
                let (tx_kind, data) = match tx {
                    TxEnvelope::Legacy(tx) => (tx.tx().to(), tx.tx().input()),
                    TxEnvelope::Eip2930(tx) => (tx.tx().to(), tx.tx().input()),
                    TxEnvelope::Eip1559(tx) => (tx.tx().to(), tx.tx().input()),
                    _ => return None,
                };
                let to = tx_kind?;

                if to != self.batch_inbox_address {
                    return None;
                }
                if tx.recover_signer().ok()? != batcher_address {
                    return None;
                }

                // Filter out reverted transactions if receipt information is available
                if let Some(ref success_map) = tx_success_map {
                    let tx_hash = tx.tx_hash();
                    if let Some(success) = success_map.get(&tx_hash) {
                        if !success {
                            return None;
                        }
                    }
                }

                Some(data.to_vec().into())
            })
            .collect::<VecDeque<_>>();

        #[cfg(feature = "metrics")]
        metrics::gauge!(
            crate::metrics::Metrics::PIPELINE_DATA_AVAILABILITY_PROVIDER,
            "source" => "calldata",
        )
        .increment(self.calldata.len() as f64);

        self.open = true;

        Ok(())
    }
}

#[async_trait]
impl<CP: ChainProvider + Send> DataAvailabilityProvider for CalldataSource<CP> {
    type Item = Bytes;

    async fn next(
        &mut self,
        block_ref: &BlockInfo,
        batcher_address: Address,
    ) -> PipelineResult<Self::Item> {
        self.load_calldata(block_ref, batcher_address).await.map_err(Into::into)?;
        self.calldata.pop_front().ok_or(PipelineError::Eof.temp())
    }

    fn clear(&mut self) {
        self.calldata.clear();
        self.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{errors::PipelineErrorKind, test_utils::TestChainProvider};
    use alloc::{vec, vec::Vec};
    use alloy_consensus::{Signed, TxEip2930, TxEip4844, TxEip4844Variant, TxEip7702, TxLegacy};
    use alloy_primitives::{Address, Signature, TxKind, address};

    pub(crate) fn test_legacy_tx(to: Address) -> TxEnvelope {
        let sig = Signature::test_signature();
        TxEnvelope::Legacy(Signed::new_unchecked(
            TxLegacy { to: TxKind::Call(to), ..Default::default() },
            sig,
            Default::default(),
        ))
    }

    pub(crate) fn test_eip2930_tx(to: Address) -> TxEnvelope {
        let sig = Signature::test_signature();
        TxEnvelope::Eip2930(Signed::new_unchecked(
            TxEip2930 { to: TxKind::Call(to), ..Default::default() },
            sig,
            Default::default(),
        ))
    }

    pub(crate) fn test_eip7702_tx(to: Address) -> TxEnvelope {
        let sig = Signature::test_signature();
        TxEnvelope::Eip7702(Signed::new_unchecked(
            TxEip7702 { to, ..Default::default() },
            sig,
            Default::default(),
        ))
    }

    pub(crate) fn test_blob_tx(to: Address) -> TxEnvelope {
        let sig = Signature::test_signature();
        TxEnvelope::Eip4844(Signed::new_unchecked(
            TxEip4844Variant::TxEip4844(TxEip4844 { to, ..Default::default() }),
            sig,
            Default::default(),
        ))
    }

    pub(crate) fn default_test_calldata_source() -> CalldataSource<TestChainProvider> {
        CalldataSource::new(TestChainProvider::default(), Default::default())
    }

    #[tokio::test]
    async fn test_clear_calldata() {
        let mut source = default_test_calldata_source();
        source.open = true;
        source.calldata.push_back(Bytes::default());
        source.clear();
        assert!(source.calldata.is_empty());
        assert!(!source.open);
    }

    #[tokio::test]
    async fn test_load_calldata_open() {
        let mut source = default_test_calldata_source();
        source.open = true;
        assert!(source.load_calldata(&BlockInfo::default(), Address::ZERO).await.is_ok());
    }

    #[tokio::test]
    async fn test_load_calldata_provider_err() {
        let mut source = default_test_calldata_source();
        assert!(source.load_calldata(&BlockInfo::default(), Address::ZERO).await.is_err());
    }

    #[tokio::test]
    async fn test_load_calldata_chain_provider_empty_txs() {
        let mut source = default_test_calldata_source();
        let block_info = BlockInfo::default();
        source.chain_provider.insert_block_with_transactions(0, block_info, Vec::new());
        assert!(!source.open); // Source is not open by default.
        assert!(source.load_calldata(&BlockInfo::default(), Address::ZERO).await.is_ok());
        assert!(source.calldata.is_empty());
        assert!(source.open);
    }

    #[tokio::test]
    async fn test_load_calldata_wrong_batch_inbox_address() {
        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        let block_info = BlockInfo::default();
        let tx = test_legacy_tx(batch_inbox_address);
        source.chain_provider.insert_block_with_transactions(0, block_info, vec![tx]);
        assert!(!source.open); // Source is not open by default.
        assert!(source.load_calldata(&BlockInfo::default(), Address::ZERO).await.is_ok());
        assert!(source.calldata.is_empty());
        assert!(source.open);
    }

    #[tokio::test]
    async fn test_load_calldata_wrong_signer() {
        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        source.batch_inbox_address = batch_inbox_address;
        let block_info = BlockInfo::default();
        let tx = test_legacy_tx(batch_inbox_address);
        source.chain_provider.insert_block_with_transactions(0, block_info, vec![tx]);
        assert!(!source.open); // Source is not open by default.
        assert!(source.load_calldata(&BlockInfo::default(), Address::ZERO).await.is_ok());
        assert!(source.calldata.is_empty());
        assert!(source.open);
    }

    #[tokio::test]
    async fn test_load_calldata_valid_legacy_tx() {
        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        source.batch_inbox_address = batch_inbox_address;
        let tx = test_legacy_tx(batch_inbox_address);
        let block_info = BlockInfo::default();
        source.chain_provider.insert_block_with_transactions(0, block_info, vec![tx.clone()]);
        assert!(!source.open); // Source is not open by default.
        assert!(
            source.load_calldata(&BlockInfo::default(), tx.recover_signer().unwrap()).await.is_ok()
        );
        assert!(!source.calldata.is_empty()); // Calldata is NOT empty.
        assert!(source.open);
    }

    #[tokio::test]
    async fn test_load_calldata_valid_eip2930_tx() {
        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        source.batch_inbox_address = batch_inbox_address;
        let tx = test_eip2930_tx(batch_inbox_address);
        let block_info = BlockInfo::default();
        source.chain_provider.insert_block_with_transactions(0, block_info, vec![tx.clone()]);
        assert!(!source.open); // Source is not open by default.
        assert!(
            source.load_calldata(&BlockInfo::default(), tx.recover_signer().unwrap()).await.is_ok()
        );
        assert!(!source.calldata.is_empty()); // Calldata is NOT empty.
        assert!(source.open);
    }

    #[tokio::test]
    async fn test_load_calldata_blob_tx_ignored() {
        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        source.batch_inbox_address = batch_inbox_address;
        let tx = test_blob_tx(batch_inbox_address);
        let block_info = BlockInfo::default();
        source.chain_provider.insert_block_with_transactions(0, block_info, vec![tx.clone()]);
        assert!(!source.open); // Source is not open by default.
        assert!(
            source.load_calldata(&BlockInfo::default(), tx.recover_signer().unwrap()).await.is_ok()
        );
        assert!(source.calldata.is_empty());
        assert!(source.open);
    }

    #[tokio::test]
    async fn test_load_calldata_eip7702_tx_ignored() {
        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        source.batch_inbox_address = batch_inbox_address;
        let tx = test_eip7702_tx(batch_inbox_address);
        let block_info = BlockInfo::default();
        source.chain_provider.insert_block_with_transactions(0, block_info, vec![tx.clone()]);
        assert!(!source.open); // Source is not open by default.
        assert!(
            source.load_calldata(&BlockInfo::default(), tx.recover_signer().unwrap()).await.is_ok()
        );
        assert!(source.calldata.is_empty());
        assert!(source.open);
    }

    #[tokio::test]
    async fn test_next_err_loading_calldata() {
        let mut source = default_test_calldata_source();
        assert!(matches!(
            source.next(&BlockInfo::default(), Address::ZERO).await,
            Err(PipelineErrorKind::Temporary(_))
        ));
    }

    #[tokio::test]
    async fn test_load_calldata_filters_reverted_transactions() {
        use alloy_consensus::Receipt;
        use alloy_primitives::address;

        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        source.batch_inbox_address = batch_inbox_address;
        
        let block_info = BlockInfo::default();
        
        // Create one successful transaction
        let successful_tx = test_legacy_tx(batch_inbox_address);
        
        // Insert just the successful transaction
        source.chain_provider.insert_block_with_transactions(
            0, 
            block_info, 
            vec![successful_tx.clone()]
        );
        
        // Create a reverted receipt for this transaction to test filtering
        let reverted_receipt = Receipt {
            status: alloy_consensus::Eip658Value::Eip658(false),
            cumulative_gas_used: 21000,
            logs: vec![],
        };
        
        source.chain_provider.insert_receipts(
            block_info.hash,
            vec![reverted_receipt]
        );
        
        // Load calldata with the batcher address
        let batcher_address = successful_tx.recover_signer().unwrap();
        assert!(source.load_calldata(&block_info, batcher_address).await.is_ok());
        
        // Should contain no transactions because the only transaction reverted
        assert_eq!(source.calldata.len(), 0);
        assert!(source.open);
    }

    #[tokio::test]
    async fn test_load_calldata_includes_successful_transactions() {
        use alloy_consensus::Receipt;
        use alloy_primitives::address;

        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        source.batch_inbox_address = batch_inbox_address;
        
        let block_info = BlockInfo::default();
        let successful_tx1 = test_legacy_tx(batch_inbox_address);
        let successful_tx2 = test_legacy_tx(batch_inbox_address);
        
        // Insert transactions
        source.chain_provider.insert_block_with_transactions(
            0, 
            block_info, 
            vec![successful_tx1.clone(), successful_tx2.clone()]
        );
        
        // Create receipts - both successful
        let successful_receipt1 = Receipt {
            status: alloy_consensus::Eip658Value::Eip658(true),
            cumulative_gas_used: 21000,
            logs: vec![],
        };
        let successful_receipt2 = Receipt {
            status: alloy_consensus::Eip658Value::Eip658(true),
            cumulative_gas_used: 42000,
            logs: vec![],
        };
        
        source.chain_provider.insert_receipts(
            block_info.hash,
            vec![successful_receipt1, successful_receipt2]
        );
        
        // Load calldata with the batcher address
        let batcher_address = successful_tx1.recover_signer().unwrap();
        assert!(source.load_calldata(&block_info, batcher_address).await.is_ok());
        
        // Should contain data from both successful transactions
        assert_eq!(source.calldata.len(), 2);
        assert!(source.open);
    }

    #[tokio::test]
    async fn test_load_calldata_handles_missing_receipts() {
        use alloy_primitives::address;

        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        source.batch_inbox_address = batch_inbox_address;
        
        let block_info = BlockInfo::default();
        let tx = test_legacy_tx(batch_inbox_address);
        
        // Insert transaction but no receipts
        source.chain_provider.insert_block_with_transactions(0, block_info, vec![tx.clone()]);
        
        // Load calldata should succeed even without receipts (backward compatibility)
        let batcher_address = tx.recover_signer().unwrap();
        assert!(source.load_calldata(&block_info, batcher_address).await.is_ok());
        assert_eq!(source.calldata.len(), 1); // Should include the transaction
    }

    #[tokio::test]
    async fn test_debug_transaction_filtering() {
        use alloy_consensus::{Receipt, Signed, TxLegacy};
        use alloy_primitives::{address, Signature, TxKind};

        let batch_inbox_address = address!("0123456789012345678901234567890123456789");
        let mut source = default_test_calldata_source();
        source.batch_inbox_address = batch_inbox_address;
        
        let block_info = BlockInfo::default();
        
        // Create a single transaction to debug
        let tx = TxEnvelope::Legacy(Signed::new_unchecked(
            TxLegacy { to: TxKind::Call(batch_inbox_address), nonce: 0, ..Default::default() },
            Signature::test_signature(),
            Default::default(),
        ));
        
        // Insert transaction
        source.chain_provider.insert_block_with_transactions(0, block_info, vec![tx.clone()]);
        
        // Create a successful receipt
        let successful_receipt = Receipt {
            status: alloy_consensus::Eip658Value::Eip658(true),
            cumulative_gas_used: 21000,
            logs: vec![],
        };
        
        source.chain_provider.insert_receipts(block_info.hash, vec![successful_receipt]);
        
        // Load calldata with the batcher address
        let batcher_address = tx.recover_signer().unwrap();
        assert!(source.load_calldata(&block_info, batcher_address).await.is_ok());
        assert_eq!(source.calldata.len(), 1);
    }
}
