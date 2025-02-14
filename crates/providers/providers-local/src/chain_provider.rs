//! Chain Provider

use alloc::{boxed::Box, collections::vec_deque::VecDeque, string::ToString, sync::Arc, vec::Vec};
use alloy_primitives::{map::HashMap, PrimitiveSignature, B256};

use alloy_consensus::{
    Header, Receipt, Signed, TxEip1559, TxEip2930, TxEip4844, TxEip4844Variant, TxEnvelope,
    TxLegacy,
};
use alloy_eips::BlockNumHash;
use async_trait::async_trait;
use kona_derive::{
    errors::{PipelineError, PipelineErrorKind},
    traits::ChainProvider,
};
use kona_protocol::BlockInfo;
use parking_lot::RwLock;
use reth_primitives::Transaction;
use reth_provider::Chain;

/// An in-memory [ChainProvider] that stores chain data,
/// meant to be shared between multiple readers.
///
/// This provider uses a ring buffer to limit capacity
/// to avoid storing an unbounded amount of data in memory.
#[derive(Debug, Clone)]
pub struct InMemoryChainProvider(Arc<RwLock<InMemoryChainProviderInner>>);

impl InMemoryChainProvider {
    /// Create a new [InMemoryChainProvider] with the given capacity.
    pub fn with_capacity(cap: usize) -> Self {
        Self(Arc::new(RwLock::new(InMemoryChainProviderInner::with_capacity(cap))))
    }

    /// Commits Chain state to the provider.
    pub fn commit(&mut self, chain: Arc<Chain>) {
        self.0.write().commit(chain);
    }

    /// Inserts the L2 genesis [BlockNumHash] into the provider.
    pub fn insert_l2_genesis_block(&mut self, block: BlockNumHash) {
        self.0.write().insert_l2_genesis_block(block);
    }

    /// Flush the chain provider.
    pub fn flush(&mut self) {
        self.0.write().hash_to_header.clear();
        self.0.write().hash_to_block_info.clear();
        self.0.write().hash_to_receipts.clear();
        self.0.write().hash_to_txs.clear();
    }
}

/// The inner state of an [InMemoryChainProvider].
#[derive(Debug)]
struct InMemoryChainProviderInner {
    /// The maximum number of items to store in the provider.
    /// This is used to prevent unbounded memory usage.
    capacity: usize,

    /// The order in which keys were inserted into the provider.
    /// This is used to evict the oldest items when the provider
    /// reaches its capacity.
    key_order: VecDeque<B256>,

    /// Maps [B256] hash to [Header].
    hash_to_header: HashMap<B256, Header>,

    /// Maps [B256] hash to [BlockInfo].
    hash_to_block_info: HashMap<B256, BlockInfo>,

    /// Maps [B256] hash to [Vec]<[Receipt]>.
    hash_to_receipts: HashMap<B256, Vec<Receipt>>,

    /// Maps a [B256] hash to a [Vec]<[TxEnvelope]>.
    hash_to_txs: HashMap<B256, Vec<TxEnvelope>>,
}

impl InMemoryChainProviderInner {
    fn with_capacity(cap: usize) -> Self {
        Self {
            capacity: cap,
            key_order: VecDeque::new(),
            hash_to_header: HashMap::default(),
            hash_to_block_info: HashMap::default(),
            hash_to_receipts: HashMap::default(),
            hash_to_txs: HashMap::default(),
        }
    }

    /// Commits Chain state to the provider.
    fn commit(&mut self, chain: Arc<Chain>) {
        // Remove the oldest items if the provider is at capacity.
        self.key_order.extend(chain.headers().map(|h| h.hash()));
        if self.key_order.len() > self.capacity {
            let to_remove = self.key_order.len() - self.capacity;
            for _ in 0..to_remove {
                if let Some(key) = self.key_order.pop_front() {
                    self.hash_to_header.remove(&key);
                    self.hash_to_block_info.remove(&key);
                    self.hash_to_receipts.remove(&key);
                    self.hash_to_txs.remove(&key);
                }
            }
        }

        self.commit_headers(&chain);
        self.commit_block_infos(&chain);
        self.commit_receipts(&chain);
        self.commit_txs(&chain);
    }

    /// Commits [Header]s to the provider.
    fn commit_headers(&mut self, chain: &Arc<Chain>) {
        for header in chain.headers() {
            self.hash_to_header.insert(
                header.hash(),
                Header {
                    parent_hash: header.parent_hash,
                    ommers_hash: header.ommers_hash,
                    beneficiary: header.beneficiary,
                    state_root: header.state_root,
                    transactions_root: header.transactions_root,
                    receipts_root: header.receipts_root,
                    logs_bloom: header.logs_bloom,
                    difficulty: header.difficulty,
                    number: header.number,
                    gas_limit: header.gas_limit,
                    gas_used: header.gas_used,
                    timestamp: header.timestamp,
                    extra_data: header.extra_data.clone(),
                    mix_hash: header.mix_hash,
                    nonce: header.nonce,
                    base_fee_per_gas: header.base_fee_per_gas,
                    withdrawals_root: header.withdrawals_root,
                    blob_gas_used: header.blob_gas_used,
                    excess_blob_gas: header.excess_blob_gas,
                    parent_beacon_block_root: header.parent_beacon_block_root,
                    requests_hash: header.requests_root,
                },
            );
        }
    }

    /// Commits [BlockInfo]s to the provider.
    fn commit_block_infos(&mut self, chain: &Arc<Chain>) {
        for block in chain.blocks_iter() {
            self.hash_to_block_info.insert(
                block.hash(),
                BlockInfo {
                    hash: block.hash(),
                    number: block.number,
                    timestamp: block.timestamp,
                    parent_hash: block.parent_hash,
                },
            );
        }
    }

    /// Inserts the L2 genesis [BlockNumHash] into the provider.
    fn insert_l2_genesis_block(&mut self, block: BlockNumHash) {
        self.hash_to_block_info.insert(
            block.hash,
            BlockInfo {
                hash: block.hash,
                number: block.number,
                timestamp: Default::default(),
                parent_hash: Default::default(),
            },
        );
    }

    /// Commits [Receipt]s to the provider.
    fn commit_receipts(&mut self, chain: &Arc<Chain>) {
        for (b, receipt) in chain.blocks_and_receipts() {
            self.hash_to_receipts.insert(
                b.hash(),
                receipt
                    .iter()
                    .flat_map(|r| {
                        r.as_ref().map(|r| Receipt {
                            cumulative_gas_used: r.cumulative_gas_used,
                            logs: r.logs.clone(),
                            status: alloy_consensus::Eip658Value::Eip658(r.success),
                        })
                    })
                    .collect(),
            );
        }
    }

    /// Commits [TxEnvelope]s to the provider.
    fn commit_txs(&mut self, chain: &Arc<Chain>) {
        for b in chain.blocks_iter() {
            let txs = b.transactions().flat_map(reth_to_alloy_tx).collect();
            self.hash_to_txs.insert(b.hash(), txs);
        }
    }
}

/// An error for the [InMemoryChainProvider].
#[derive(Debug, derive_more::Display)]
pub enum InMemoryChainProviderError {
    /// The header is not set.
    #[display("Header not set")]
    HeaderNotSet,
    /// The block info is not set.
    #[display("Block info not set")]
    BlockInfoNotSet,
    /// The receipts are not set.
    #[display("Receipts not set")]
    ReceiptsNotSet,
    /// The transactions are not set.
    #[display("Transactions not set")]
    TxsNotSet,
    /// The block is not found.
    #[display("Block not found")]
    BlockNotFound,
}

impl core::error::Error for InMemoryChainProviderError {}

impl From<InMemoryChainProviderError> for PipelineErrorKind {
    fn from(e: InMemoryChainProviderError) -> Self {
        match e {
            InMemoryChainProviderError::BlockNotFound => {
                PipelineErrorKind::Temporary(PipelineError::Provider("block not found".to_string()))
            }
            InMemoryChainProviderError::HeaderNotSet => {
                PipelineErrorKind::Temporary(PipelineError::Provider("header not set".to_string()))
            }
            InMemoryChainProviderError::BlockInfoNotSet => PipelineErrorKind::Temporary(
                PipelineError::Provider("block info not set".to_string()),
            ),
            InMemoryChainProviderError::ReceiptsNotSet => PipelineErrorKind::Temporary(
                PipelineError::Provider("receipts not set".to_string()),
            ),
            InMemoryChainProviderError::TxsNotSet => PipelineErrorKind::Temporary(
                PipelineError::Provider("transactions not set".to_string()),
            ),
        }
    }
}

#[async_trait]
impl ChainProvider for InMemoryChainProvider {
    type Error = InMemoryChainProviderError;

    /// Fetch the L1 [Header] for the given [B256] hash.
    async fn header_by_hash(&mut self, hash: B256) -> Result<Header, Self::Error> {
        self.0
            .read()
            .hash_to_header
            .get(&hash)
            .cloned()
            .ok_or_else(|| InMemoryChainProviderError::HeaderNotSet)
    }

    /// Returns the block at the given number, or an error if the block does not exist in the data
    /// source.
    async fn block_info_by_number(&mut self, number: u64) -> Result<BlockInfo, Self::Error> {
        self.0
            .read()
            .hash_to_block_info
            .values()
            .find(|bi| bi.number == number)
            .cloned()
            .ok_or_else(|| InMemoryChainProviderError::BlockInfoNotSet)
    }

    /// Returns all receipts in the block with the given hash, or an error if the block does not
    /// exist in the data source.
    async fn receipts_by_hash(&mut self, hash: B256) -> Result<Vec<Receipt>, Self::Error> {
        self.0
            .read()
            .hash_to_receipts
            .get(&hash)
            .cloned()
            .ok_or_else(|| InMemoryChainProviderError::ReceiptsNotSet)
    }

    /// Returns block info and transactions for the given block hash.
    async fn block_info_and_transactions_by_hash(
        &mut self,
        hash: B256,
    ) -> Result<(BlockInfo, Vec<TxEnvelope>), Self::Error> {
        let block_info = self
            .0
            .read()
            .hash_to_block_info
            .get(&hash)
            .cloned()
            .ok_or_else(|| InMemoryChainProviderError::BlockNotFound)?;

        let txs = self
            .0
            .read()
            .hash_to_txs
            .get(&hash)
            .cloned()
            .ok_or_else(|| InMemoryChainProviderError::TxsNotSet)?;

        Ok((block_info, txs))
    }
}

pub fn reth_to_alloy_tx(tx: &reth_primitives::TransactionSigned) -> Option<TxEnvelope> {
    let sig = PrimitiveSignature::try_from(tx.signature.as_bytes().as_slice()).ok()?;

    let new = match &tx.transaction {
        Transaction::Legacy(l) => {
            let legacy_tx = TxLegacy {
                chain_id: l.chain_id,
                nonce: l.nonce,
                gas_price: l.gas_price,
                gas_limit: l.gas_limit,
                to: l.to,
                value: l.value,
                input: l.input.clone(),
            };
            TxEnvelope::Legacy(Signed::new_unchecked(legacy_tx, sig, tx.signature_hash()))
        }
        Transaction::Eip2930(e) => {
            let eip_tx = TxEip2930 {
                chain_id: e.chain_id,
                nonce: e.nonce,
                gas_price: e.gas_price,
                gas_limit: e.gas_limit,
                to: e.to,
                value: e.value,
                input: e.input.clone(),
                access_list: alloy_eips::eip2930::AccessList(
                    e.access_list
                        .0
                        .clone()
                        .into_iter()
                        .map(|item| alloy_eips::eip2930::AccessListItem {
                            address: item.address,
                            storage_keys: item.storage_keys.clone(),
                        })
                        .collect(),
                ),
            };
            TxEnvelope::Eip2930(Signed::new_unchecked(eip_tx, sig, tx.signature_hash()))
        }
        Transaction::Eip1559(e) => {
            let eip_tx = TxEip1559 {
                chain_id: e.chain_id,
                nonce: e.nonce,
                max_priority_fee_per_gas: e.max_priority_fee_per_gas,
                max_fee_per_gas: e.max_fee_per_gas,
                gas_limit: e.gas_limit,
                to: e.to,
                value: e.value,
                input: e.input.clone(),
                access_list: alloy_eips::eip2930::AccessList(
                    e.access_list
                        .0
                        .clone()
                        .into_iter()
                        .map(|item| alloy_eips::eip2930::AccessListItem {
                            address: item.address,
                            storage_keys: item.storage_keys.clone(),
                        })
                        .collect(),
                ),
            };
            TxEnvelope::Eip1559(Signed::new_unchecked(eip_tx, sig, tx.signature_hash()))
        }
        Transaction::Eip4844(e) => {
            let eip_tx = TxEip4844 {
                chain_id: e.chain_id,
                nonce: e.nonce,
                max_fee_per_gas: e.max_fee_per_gas,
                max_priority_fee_per_gas: e.max_priority_fee_per_gas,
                max_fee_per_blob_gas: e.max_fee_per_blob_gas,
                blob_versioned_hashes: e.blob_versioned_hashes.clone(),
                gas_limit: e.gas_limit,
                to: e.to,
                value: e.value,
                input: e.input.clone(),
                access_list: alloy_eips::eip2930::AccessList(
                    e.access_list
                        .0
                        .clone()
                        .into_iter()
                        .map(|item| alloy_eips::eip2930::AccessListItem {
                            address: item.address,
                            storage_keys: item.storage_keys.clone(),
                        })
                        .collect(),
                ),
            };
            TxEnvelope::Eip4844(Signed::new_unchecked(
                TxEip4844Variant::TxEip4844(eip_tx),
                sig,
                tx.signature_hash(),
            ))
        }
        Transaction::Eip7702(_) => unimplemented!("EIP-7702 not implemented"),
    };
    Some(new)
}
