//! [InteropProvider] trait implementation using a [CommsClient] data source.

use crate::{HintType, PreState};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use alloy_consensus::Header;
use alloy_eips::eip2718::Decodable2718;
use alloy_primitives::B256;
use alloy_rlp::Decodable;
use async_trait::async_trait;
use kona_interop::InteropProvider;
use kona_mpt::{OrderedListWalker, TrieNode, TrieProvider};
use kona_preimage::{CommsClient, PreimageKey, PreimageKeyType};
use kona_proof::errors::OracleProviderError;
use maili_registry::HashMap;
use op_alloy_consensus::OpReceiptEnvelope;
use spin::RwLock;

/// A [CommsClient] backed [InteropProvider] implementation.
#[derive(Debug, Clone)]
pub struct OracleInteropProvider<T> {
    /// The oracle client.
    oracle: Arc<T>,
    /// The [PreState] for the current program execution.
    pre_state: PreState,
    /// The safe head block header cache, keyed by chain ID.
    safe_head_cache: Arc<RwLock<HashMap<u64, Header>>>,
}

impl<T> OracleInteropProvider<T>
where
    T: CommsClient + Send + Sync,
{
    /// Creates a new [OracleInteropProvider] with the given oracle client and [PreState].
    pub fn new(oracle: Arc<T>, pre_state: PreState) -> Self {
        Self { oracle, pre_state, safe_head_cache: Arc::new(RwLock::new(HashMap::default())) }
    }

    /// Fetch the [Header] for the block with the given hash.
    pub async fn header_by_hash(
        &self,
        chain_id: u64,
        block_hash: B256,
    ) -> Result<Header, <Self as InteropProvider>::Error> {
        self.oracle
            .write(
                &HintType::L2BlockHeader
                    .encode_with(&[block_hash.as_slice(), chain_id.to_be_bytes().as_ref()]),
            )
            .await
            .map_err(OracleProviderError::Preimage)?;

        let header_rlp = self
            .oracle
            .get(PreimageKey::new(*block_hash, PreimageKeyType::Keccak256))
            .await
            .map_err(OracleProviderError::Preimage)?;

        Header::decode(&mut header_rlp.as_ref()).map_err(OracleProviderError::Rlp)
    }

    /// Fetch the [OpReceiptEnvelope]s for the block with the given hash.
    async fn derive_receipts(
        &self,
        chain_id: u64,
        block_hash: B256,
        header: &Header,
    ) -> Result<Vec<OpReceiptEnvelope>, <Self as InteropProvider>::Error> {
        // Send a hint for the block's receipts, and walk through the receipts trie in the header to
        // verify them.
        self.oracle
            .write(
                &HintType::L2Receipts
                    .encode_with(&[block_hash.as_ref(), chain_id.to_be_bytes().as_slice()]),
            )
            .await
            .map_err(OracleProviderError::Preimage)?;
        let trie_walker = OrderedListWalker::try_new_hydrated(header.receipts_root, self)
            .map_err(OracleProviderError::TrieWalker)?;

        // Decode the receipts within the receipts trie.
        let receipts = trie_walker
            .into_iter()
            .map(|(_, rlp)| {
                let envelope = OpReceiptEnvelope::decode_2718(&mut rlp.as_ref())?;
                Ok(envelope)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(OracleProviderError::Rlp)?;

        Ok(receipts)
    }
}

#[async_trait]
impl<T> InteropProvider for OracleInteropProvider<T>
where
    T: CommsClient + Send + Sync,
{
    type Error = OracleProviderError;

    /// Fetch a [Header] by its number.
    async fn header_by_number(&self, chain_id: u64, number: u64) -> Result<Header, Self::Error> {
        // Find the safe head for the given chain ID.
        //
        // If the safe head is not in the cache, we need to fetch it from the oracle.
        let mut header = if let Some(header) = self.safe_head_cache.read().get(&chain_id) {
            header.clone()
        } else {
            let pre_state = match &self.pre_state {
                PreState::SuperRoot(super_root) => super_root,
                PreState::TransitionState(transition_state) => &transition_state.pre_state,
            };
            let output = pre_state
                .output_roots
                .iter()
                .find(|o| o.chain_id == chain_id)
                .ok_or(OracleProviderError::UnknownChainId(chain_id))?;
            self.oracle
                .write(&HintType::L2OutputRoot.encode_with(&[
                    output.output_root.as_slice(),
                    output.chain_id.to_be_bytes().as_slice(),
                ]))
                .await
                .map_err(OracleProviderError::Preimage)?;
            let output_preimage = self
                .oracle
                .get(PreimageKey::new(*output.output_root, PreimageKeyType::Keccak256))
                .await
                .map_err(OracleProviderError::Preimage)?;
            let safe_head_hash = output_preimage[96..128]
                .try_into()
                .map_err(OracleProviderError::SliceConversion)?;

            // Fetch the starting block header.
            let header = self.header_by_hash(chain_id, safe_head_hash).await?;
            self.safe_head_cache.write().insert(chain_id, header.clone());
            header
        };

        // Check if the block number is in range. If not, we can fail early.
        if number > header.number {
            return Err(OracleProviderError::BlockNumberPastHead(number, header.number));
        }

        // Walk back the block headers to the desired block number.
        while header.number > number {
            header = self.header_by_hash(chain_id, header.parent_hash).await?;
        }

        Ok(header)
    }

    /// Fetch all receipts for a given block by number.
    async fn receipts_by_number(
        &self,
        chain_id: u64,
        number: u64,
    ) -> Result<Vec<OpReceiptEnvelope>, Self::Error> {
        let header = self.header_by_number(chain_id, number).await?;
        self.derive_receipts(chain_id, header.hash_slow(), &header).await
    }

    /// Fetch all receipts for a given block by hash.
    async fn receipts_by_hash(
        &self,
        chain_id: u64,
        block_hash: B256,
    ) -> Result<Vec<OpReceiptEnvelope>, Self::Error> {
        let header = self.header_by_hash(chain_id, block_hash).await?;
        self.derive_receipts(chain_id, block_hash, &header).await
    }
}

impl<T> TrieProvider for OracleInteropProvider<T>
where
    T: CommsClient + Send + Sync + Clone,
{
    type Error = OracleProviderError;

    fn trie_node_by_hash(&self, key: B256) -> Result<TrieNode, Self::Error> {
        kona_proof::block_on(async move {
            let trie_node_rlp = self
                .oracle
                .get(PreimageKey::new(*key, PreimageKeyType::Keccak256))
                .await
                .map_err(OracleProviderError::Preimage)?;
            TrieNode::decode(&mut trie_node_rlp.as_ref()).map_err(OracleProviderError::Rlp)
        })
    }
}
