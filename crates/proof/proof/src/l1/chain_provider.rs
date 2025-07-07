//! Contains the concrete implementation of the [ChainProvider] trait for the proof.

use crate::{HintType, errors::OracleProviderError};
use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};
use alloy_consensus::Header;
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::B256;
use alloy_rlp::Decodable;
use async_trait::async_trait;
use soon_derive::traits::ChainProvider;
use soon_primitives::blocks::{BlockInfo, L1BlockReceipt, L1Transaction};
use kona_mpt::{TrieNode, TrieProvider};
use kona_preimage::{CommsClient, PreimageKey, PreimageKeyType};

/// The oracle-backed L1 chain provider for the client program.
#[derive(Debug, Clone)]
pub struct OracleL1ChainProvider<T: CommsClient> {
    /// The L1 head hash.
    pub l1_head: B256,
    /// The preimage oracle client.
    pub oracle: Arc<T>,
}

impl<T: CommsClient> OracleL1ChainProvider<T> {
    /// Creates a new [OracleL1ChainProvider] with the given boot information and oracle client.
    pub const fn new(l1_head: B256, oracle: Arc<T>) -> Self {
        Self { l1_head, oracle }
    }
}

#[async_trait]
impl<T: CommsClient + Sync + Send> ChainProvider for OracleL1ChainProvider<T> {
    type Error = OracleProviderError;

    async fn block_info_by_hash(&mut self, hash: B256) -> Result<BlockInfo, Self::Error> {
        // Fetch the header RLP from the oracle.
        HintType::L1BlockHeader.with_data(&[hash.as_ref()]).send(self.oracle.as_ref()).await?;
        let header_rlp = self.oracle.get(PreimageKey::new_keccak256(*hash)).await?;

        // Decode the header RLP into a Header.
        let header = Header::decode(&mut header_rlp.as_slice()).map_err(OracleProviderError::Rlp)?;
        Ok(BlockInfo {
            hash: header.hash_slow(),
            number: header.number,
            parent_hash: header.parent_hash,
            timestamp: header.timestamp,
        })

    }

    async fn block_info_by_number(&self, _block_number: BlockNumberOrTag) -> Result<BlockInfo, Self::Error> {
        Ok(BlockInfo::default())
    }

    async fn receipts_by_hash(&self, _hash: B256) -> Result<(Vec<L1BlockReceipt>, bool), Self::Error> {
        Ok((vec![], true))
    }

    async fn get_block_transactions_by_hash(
        &self,
        _hash: B256,
    ) -> Result<Vec<L1Transaction>, Self::Error> {
        Ok(vec![])
    }
}

impl<T: CommsClient> TrieProvider for OracleL1ChainProvider<T> {
    type Error = OracleProviderError;

    fn trie_node_by_hash(&self, key: B256) -> Result<TrieNode, Self::Error> {
        // On L1, trie node preimages are stored as keccak preimage types in the oracle. We assume
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
