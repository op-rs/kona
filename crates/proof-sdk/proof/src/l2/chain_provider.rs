//! Contains the concrete implementation of the [L2ChainProvider] trait for the client program.

use crate::{errors::OracleProviderError, HintType};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use alloy_consensus::{BlockBody, Header};
use alloy_eips::eip2718::Decodable2718;
use alloy_primitives::{Address, Bytes, B256};
use alloy_rlp::Decodable;
use async_trait::async_trait;
use kona_derive::traits::L2ChainProvider;
use kona_driver::PipelineCursor;
use kona_executor::TrieDBProvider;
use kona_mpt::{OrderedListWalker, TrieHinter, TrieNode, TrieProvider};
use kona_preimage::{CommsClient, PreimageKey, PreimageKeyType};
use maili_genesis::{RollupConfig, SystemConfig};
use maili_protocol::{to_system_config, BatchValidationProvider, L2BlockInfo};
use op_alloy_consensus::{OpBlock, OpTxEnvelope};
use spin::RwLock;

/// The oracle-backed L2 chain provider for the client program.
#[derive(Debug, Clone)]
pub struct OracleL2ChainProvider<T: CommsClient> {
    /// The L2 safe head block hash.
    l2_head: B256,
    /// The rollup configuration.
    rollup_config: RollupConfig,
    /// The preimage oracle client.
    oracle: Arc<T>,
    /// The derivation pipeline cursor
    cursor: Option<Arc<RwLock<PipelineCursor>>>,
}

impl<T: CommsClient> OracleL2ChainProvider<T> {
    /// Creates a new [OracleL2ChainProvider] with the given boot information and oracle client.
    pub const fn new(l2_head: B256, rollup_config: RollupConfig, oracle: Arc<T>) -> Self {
        Self { l2_head, rollup_config, oracle, cursor: None }
    }

    /// Updates the derivation pipeline cursor
    pub fn set_cursor(&mut self, cursor: Arc<RwLock<PipelineCursor>>) {
        self.cursor = Some(cursor);
    }

    /// Fetches the latest known safe head block hash according to the derivation pipeline cursor
    /// or uses the initial l2_head value if no cursor is set.
    pub async fn l2_safe_head(&self) -> Result<B256, OracleProviderError> {
        self.cursor
            .as_ref()
            .map_or(Ok(self.l2_head), |cursor| Ok(cursor.read().l2_safe_head().block_info.hash))
    }
}

impl<T: CommsClient> OracleL2ChainProvider<T> {
    /// Returns a [Header] corresponding to the given L2 block number, by walking back from the
    /// L2 safe head.
    async fn header_by_number(&mut self, block_number: u64) -> Result<Header, OracleProviderError> {
        // Fetch the starting block header.
        let mut header = self.header_by_hash(self.l2_safe_head().await?)?;

        // Check if the block number is in range. If not, we can fail early.
        if block_number > header.number {
            return Err(OracleProviderError::BlockNumberPastHead(block_number, header.number));
        }

        // Walk back the block headers to the desired block number.
        while header.number > block_number {
            header = self.header_by_hash(header.parent_hash)?;
        }

        Ok(header)
    }
}

#[async_trait]
impl<T: CommsClient + Send + Sync> BatchValidationProvider for OracleL2ChainProvider<T> {
    type Error = OracleProviderError;

    async fn l2_block_info_by_number(&mut self, number: u64) -> Result<L2BlockInfo, Self::Error> {
        // Get the block at the given number.
        let block = self.block_by_number(number).await?;

        // Construct the system config from the payload.
        L2BlockInfo::from_block_and_genesis(&block, &self.rollup_config.genesis)
            .map_err(OracleProviderError::BlockInfo)
    }

    async fn block_by_number(&mut self, number: u64) -> Result<OpBlock, Self::Error> {
        // Fetch the header for the given block number.
        let header @ Header { transactions_root, timestamp, .. } =
            self.header_by_number(number).await?;
        let header_hash = header.hash_slow();

        // Fetch the transactions in the block.
        self.oracle
            .write(&HintType::L2Transactions.encode_with(&[header_hash.as_ref()]))
            .await
            .map_err(OracleProviderError::Preimage)?;
        let trie_walker = OrderedListWalker::try_new_hydrated(transactions_root, self)
            .map_err(OracleProviderError::TrieWalker)?;

        // Decode the transactions within the transactions trie.
        let transactions = trie_walker
            .into_iter()
            .map(|(_, rlp)| {
                let res = OpTxEnvelope::decode_2718(&mut rlp.as_ref())?;
                Ok(res)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(OracleProviderError::Rlp)?;

        let optimism_block = OpBlock {
            header,
            body: BlockBody {
                transactions,
                ommers: Vec::new(),
                withdrawals: self
                    .rollup_config
                    .is_canyon_active(timestamp)
                    .then(|| alloy_eips::eip4895::Withdrawals::new(Vec::new())),
            },
        };
        Ok(optimism_block)
    }
}

#[async_trait]
impl<T: CommsClient + Send + Sync> L2ChainProvider for OracleL2ChainProvider<T> {
    type Error = OracleProviderError;

    async fn system_config_by_number(
        &mut self,
        number: u64,
        rollup_config: Arc<RollupConfig>,
    ) -> Result<SystemConfig, <Self as L2ChainProvider>::Error> {
        // Get the block at the given number.
        let block = self.block_by_number(number).await?;

        // Construct the system config from the payload.
        to_system_config(&block, rollup_config.as_ref())
            .map_err(OracleProviderError::OpBlockConversion)
    }
}

impl<T: CommsClient> TrieProvider for OracleL2ChainProvider<T> {
    type Error = OracleProviderError;

    fn trie_node_by_hash(&self, key: B256) -> Result<TrieNode, OracleProviderError> {
        // On L2, trie node preimages are stored as keccak preimage types in the oracle. We assume
        // that a hint for these preimages has already been sent, prior to this call.
        crate::block_on(async move {
            TrieNode::decode(
                &mut self
                    .oracle
                    .get(PreimageKey::new(*key, PreimageKeyType::Keccak256))
                    .await
                    .map_err(OracleProviderError::Preimage)?
                    .as_ref(),
            )
            .map_err(OracleProviderError::Rlp)
        })
    }
}

impl<T: CommsClient> TrieDBProvider for OracleL2ChainProvider<T> {
    fn bytecode_by_hash(&self, hash: B256) -> Result<Bytes, OracleProviderError> {
        // Fetch the bytecode preimage from the caching oracle.
        crate::block_on(async move {
            HintType::L2Code
                .get_preimage(self.oracle.as_ref(), hash, PreimageKeyType::Keccak256)
                .await
                .map(Into::into)
        })
    }

    fn header_by_hash(&self, hash: B256) -> Result<Header, OracleProviderError> {
        // Fetch the header from the caching oracle.
        crate::block_on(async move {
            let header_bytes = HintType::L2BlockHeader
                .get_preimage(self.oracle.as_ref(), hash, PreimageKeyType::Keccak256)
                .await?;

            Header::decode(&mut header_bytes.as_slice()).map_err(OracleProviderError::Rlp)
        })
    }
}

impl<T: CommsClient> TrieHinter for OracleL2ChainProvider<T> {
    type Error = OracleProviderError;

    fn hint_trie_node(&self, hash: B256) -> Result<(), Self::Error> {
        crate::block_on(async move {
            self.oracle
                .write(&HintType::L2StateNode.encode_with(&[hash.as_slice()]))
                .await
                .map_err(OracleProviderError::Preimage)
        })
    }

    fn hint_account_proof(&self, address: Address, block_number: u64) -> Result<(), Self::Error> {
        crate::block_on(async move {
            self.oracle
                .write(
                    &HintType::L2AccountProof
                        .encode_with(&[block_number.to_be_bytes().as_ref(), address.as_slice()]),
                )
                .await
                .map_err(OracleProviderError::Preimage)
        })
    }

    fn hint_storage_proof(
        &self,
        address: alloy_primitives::Address,
        slot: alloy_primitives::U256,
        block_number: u64,
    ) -> Result<(), Self::Error> {
        crate::block_on(async move {
            self.oracle
                .write(&HintType::L2AccountStorageProof.encode_with(&[
                    block_number.to_be_bytes().as_ref(),
                    address.as_slice(),
                    slot.to_be_bytes::<32>().as_ref(),
                ]))
                .await
                .map_err(OracleProviderError::Preimage)
        })
    }
}
