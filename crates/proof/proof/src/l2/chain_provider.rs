//! Contains the concrete implementation of the [L2ChainProvider] trait for the client program.

use crate::{HintType, errors::OracleProviderError};
use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};
use alloc::string::ToString;
use alloy_consensus::Header;
use alloy_primitives::{Address, B256, Bytes};
use alloy_rlp::Decodable;
use async_trait::async_trait;
use soon_derive::traits::L2ChainProvider;
use kona_driver::PipelineCursor;
use kona_executor::TrieDBProvider;
use soon_primitives::system_config::SystemConfig;
use soon_primitives::rollup_config::SoonRollupConfig;
use kona_mpt::{TrieHinter, TrieNode, TrieProvider};
use kona_preimage::{CommsClient, PreimageKey, PreimageKeyType};
use soon_primitives::blocks::L2BlockInfo;
use spin::RwLock;
use solana_transaction_status::VersionedConfirmedBlock;

/// The oracle-backed L2 chain provider for the client program.
#[derive(Debug, Clone)]
pub struct OracleL2ChainProvider<T: CommsClient> {
    /// The L2 safe head block hash.
    l2_head: B256,
    /// The rollup configuration.
    #[allow(dead_code)]
    rollup_config: Arc<SoonRollupConfig>,
    /// The preimage oracle client.
    oracle: Arc<T>,
    /// The derivation pipeline cursor
    cursor: Option<Arc<RwLock<PipelineCursor>>>,
    /// The L2 chain ID to use for the provider's hints.
    chain_id: Option<u64>,
}

impl<T: CommsClient> OracleL2ChainProvider<T> {
    /// Creates a new [OracleL2ChainProvider] with the given boot information and oracle client.
    pub const fn new(l2_head: B256, rollup_config: Arc<SoonRollupConfig>, oracle: Arc<T>) -> Self {
        Self { l2_head, rollup_config, oracle, cursor: None, chain_id: None }
    }

    /// Sets the L2 chain ID to use for the provider's hints.
    pub fn set_chain_id(&mut self, chain_id: Option<u64>) {
        self.chain_id = chain_id;
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
    #[allow(dead_code)]
    async fn header_by_number(&mut self, block_number: u64) -> Result<Header, OracleProviderError> {
        // Fetch the starting block header.
        let mut header = self.header_by_hash(self.l2_safe_head().await?)?;

        // Check if the block number is in range. If not, we can fail early.
        if block_number > header.number {
            return Err(OracleProviderError::BlockNumberPastHead(block_number, header.number));
        }

        while header.number > block_number {
            header = self.header_by_hash(header.parent_hash)?;
        }

        Ok(header)
    }
}

#[async_trait]
impl<T: CommsClient + Send + Sync> L2ChainProvider for OracleL2ChainProvider<T> {
    type Error = OracleProviderError;

    async fn l2_block_info_by_number(&mut self, _number: u64) -> Result<L2BlockInfo, Self::Error> {
        // Get the block at the given number.
        Ok(L2BlockInfo::default())
    }

    async fn block_by_number(&mut self, _number: u64) -> Result<VersionedConfirmedBlock, Self::Error> {
        Ok(VersionedConfirmedBlock {
            previous_blockhash: "".to_string(),
            blockhash: "".to_string(),
            parent_slot: 0,
            transactions: vec![],
            rewards: vec![],
            num_partitions: None,
            block_time: None,
            block_height: None,
        })
    }

    async fn system_config_by_number(
        &mut self,
        number: u64,
    ) -> Result<SystemConfig, <Self as L2ChainProvider>::Error> {
        // Get the block at the given number.
        let _block = self.block_by_number(number).await?;

        // Construct the system config from the payload.
        // to_system_config(&block, rollup_config.as_ref())
        //     .map_err(OracleProviderError::OpBlockConversion)
        //TODO block to system config impl
        Ok(SystemConfig::default())
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
                .with_data(&[hash.as_slice()])
                .with_data(self.chain_id.map_or_else(Vec::new, |id| id.to_be_bytes().to_vec()))
                .send(self.oracle.as_ref())
                .await?;
            self.oracle
                .get(PreimageKey::new_keccak256(*hash))
                .await
                .map(Into::into)
                .map_err(OracleProviderError::Preimage)
        })
    }

    fn header_by_hash(&self, hash: B256) -> Result<Header, OracleProviderError> {
        // Fetch the header from the caching oracle.
        crate::block_on(async move {
            HintType::L2BlockHeader
                .with_data(&[hash.as_slice()])
                .with_data(self.chain_id.map_or_else(Vec::new, |id| id.to_be_bytes().to_vec()))
                .send(self.oracle.as_ref())
                .await?;
            let header_bytes = self.oracle.get(PreimageKey::new_keccak256(*hash)).await?;

            Header::decode(&mut header_bytes.as_slice()).map_err(OracleProviderError::Rlp)
        })
    }
}

impl<T: CommsClient> TrieHinter for OracleL2ChainProvider<T> {
    type Error = OracleProviderError;

    fn hint_trie_node(&self, hash: B256) -> Result<(), Self::Error> {
        crate::block_on(async move {
            HintType::L2StateNode
                .with_data(&[hash.as_slice()])
                .with_data(self.chain_id.map_or_else(Vec::new, |id| id.to_be_bytes().to_vec()))
                .send(self.oracle.as_ref())
                .await
        })
    }

    fn hint_account_proof(&self, address: Address, block_number: u64) -> Result<(), Self::Error> {
        crate::block_on(async move {
            HintType::L2AccountProof
                .with_data(&[block_number.to_be_bytes().as_ref(), address.as_slice()])
                .with_data(self.chain_id.map_or_else(Vec::new, |id| id.to_be_bytes().to_vec()))
                .send(self.oracle.as_ref())
                .await
        })
    }

    fn hint_storage_proof(
        &self,
        address: alloy_primitives::Address,
        slot: alloy_primitives::U256,
        block_number: u64,
    ) -> Result<(), Self::Error> {
        crate::block_on(async move {
            HintType::L2AccountStorageProof
                .with_data(&[
                    block_number.to_be_bytes().as_ref(),
                    address.as_slice(),
                    slot.to_be_bytes::<32>().as_ref(),
                ])
                .with_data(self.chain_id.map_or_else(Vec::new, |id| id.to_be_bytes().to_vec()))
                .send(self.oracle.as_ref())
                .await
        })
    }

    fn hint_execution_witness(
        &self,
        parent_hash: B256,
        op_payload_attributes: &op_alloy_rpc_types_engine::OpPayloadAttributes,
    ) -> Result<(), Self::Error> {
        crate::block_on(async move {
            let encoded_attributes =
                serde_json::to_vec(op_payload_attributes).map_err(OracleProviderError::Serde)?;

            HintType::L2PayloadWitness
                .with_data(&[parent_hash.as_slice(), &encoded_attributes])
                .with_data(self.chain_id.map_or_else(Vec::new, |id| id.to_be_bytes().to_vec()))
                .send(self.oracle.as_ref())
                .await
        })
    }
}
