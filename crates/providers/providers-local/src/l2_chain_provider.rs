//! L2 Chain Provider

use alloc::{boxed::Box, collections::vec_deque::VecDeque, string::ToString, sync::Arc, vec::Vec};
use alloy_primitives::{map::HashMap, B256};

use alloy_consensus::{Header, Receipt, TxEnvelope};
use async_trait::async_trait;
use kona_derive::{
    errors::{PipelineError, PipelineErrorKind},
    traits::L2ChainProvider,
};
use kona_genesis::{RollupConfig, SystemConfig};
use kona_protocol::{BatchValidationProvider, BlockInfo, L2BlockInfo};
use op_alloy_consensus::OpBlock;
use parking_lot::RwLock;

/// An in-memory [L2ChainProvider].
#[derive(Debug, Clone)]
pub struct InMemoryL2ChainProvider(Arc<RwLock<InMemoryL2ChainProviderInner>>);

impl InMemoryL2ChainProvider {
    /// Create a new [InMemoryL2ChainProvider] with the given capacity.
    pub fn with_capacity(cap: usize) -> Self {
        Self(Arc::new(RwLock::new(InMemoryL2ChainProviderInner::with_capacity(cap))))
    }

    /// Flushes the provider, removing all items.
    pub fn flush(&mut self) {
        let mut inner = self.0.write();
        inner.key_order.clear();
        inner.hash_to_header.clear();
        inner.hash_to_block_info.clear();
        inner.hash_to_receipts.clear();
        inner.hash_to_txs.clear();
    }
}

/// The inner state of an [InMemoryL2ChainProvider].
#[derive(Debug)]
struct InMemoryL2ChainProviderInner {
    /// The maximum number of items to store in the provider.
    /// This is used to prevent unbounded memory usage.
    #[allow(unused)]
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

impl InMemoryL2ChainProviderInner {
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
}

/// An error that can occur when interacting with an [InMemoryL2ChainProvider].
#[derive(Debug, derive_more::Display)]
pub enum InMemoryL2ChainProviderError {
    /// The block does not exist.
    #[display("Block does not exist")]
    BlockDoesNotExist,
}

impl From<InMemoryL2ChainProviderError> for PipelineErrorKind {
    fn from(err: InMemoryL2ChainProviderError) -> Self {
        match err {
            InMemoryL2ChainProviderError::BlockDoesNotExist => PipelineErrorKind::Temporary(
                PipelineError::Provider("Block does not exist".to_string()),
            ),
        }
    }
}

impl core::error::Error for InMemoryL2ChainProviderError {}

#[async_trait]
impl BatchValidationProvider for InMemoryL2ChainProvider {
    type Error = InMemoryL2ChainProviderError;

    /// Returns the [L2BlockInfo] given a block number.
    ///
    /// Errors if the block does not exist.
    async fn l2_block_info_by_number(&mut self, _: u64) -> Result<L2BlockInfo, Self::Error> {
        todo!()
    }

    /// Returns the [OpBlock] for a given number.
    ///
    /// Errors if no block is available for the given block number.
    async fn block_by_number(&mut self, _: u64) -> Result<OpBlock, Self::Error> {
        todo!()
    }
}

#[async_trait]
impl L2ChainProvider for InMemoryL2ChainProvider {
    type Error = InMemoryL2ChainProviderError;

    /// Returns the [SystemConfig] by L2 number.
    async fn system_config_by_number(
        &mut self,
        _: u64,
        _: Arc<RollupConfig>,
    ) -> Result<SystemConfig, <Self as BatchValidationProvider>::Error> {
        todo!()
    }
}
