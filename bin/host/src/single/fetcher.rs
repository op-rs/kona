//! This module contains the [SingleChainFetcher] struct, which is responsible for fetching
//! preimages from a remote source serving the single-chain proof mode.

use crate::KeyValueStore;
use alloy_consensus::{Header, TxEnvelope, EMPTY_ROOT_HASH};
use alloy_eips::{
    eip2718::Encodable2718,
    eip4844::{IndexedBlobHash, FIELD_ELEMENTS_PER_BLOB},
    BlockId,
};
use alloy_primitives::{address, keccak256, map::HashMap, Address, Bytes, B256};
use alloy_provider::{Provider, ReqwestProvider};
use alloy_rlp::{Decodable, EMPTY_STRING_CODE};
use alloy_rpc_types::{
    debug::ExecutionWitness, Block, BlockNumberOrTag, BlockTransactions, BlockTransactionsKind,
    Transaction,
};
use anyhow::{anyhow, Result};
use kona_preimage::{
    errors::{PreimageOracleError, PreimageOracleResult},
    HintRouter, PreimageFetcher, PreimageKey, PreimageKeyType,
};
use kona_proof::{Hint, HintType};
use kona_providers_alloy::{OnlineBeaconClient, OnlineBlobProvider};
use maili_protocol::BlockInfo;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, trace, warn};

/// The [SingleChainFetcher] struct is responsible for fetching preimages from a remote source.
#[derive(Debug)]
pub struct SingleChainFetcher<KV>
where
    KV: KeyValueStore + ?Sized,
{
    /// Key-value store for preimages.
    kv_store: Arc<RwLock<KV>>,
    /// L1 chain provider.
    l1_provider: ReqwestProvider,
    /// The blob provider
    blob_provider: OnlineBlobProvider<OnlineBeaconClient>,
    /// L2 chain provider.
    l2_provider: ReqwestProvider,
    /// L2 head
    l2_head: B256,
    /// The last hint that was received. [None] if no hint has been received yet.
    last_hint: Arc<RwLock<Option<String>>>,
}

impl<KV> SingleChainFetcher<KV>
where
    KV: KeyValueStore + ?Sized,
{
    /// Create a new [SingleChainFetcher] with the given [KeyValueStore].
    pub fn new(
        kv_store: Arc<RwLock<KV>>,
        l1_provider: ReqwestProvider,
        blob_provider: OnlineBlobProvider<OnlineBeaconClient>,
        l2_provider: ReqwestProvider,
        l2_head: B256,
    ) -> Self {
        Self {
            kv_store,
            l1_provider,
            blob_provider,
            l2_provider,
            l2_head,
            last_hint: Arc::new(RwLock::new(None)),
        }
    }

    /// Fetch the preimage for the given hint and insert it into the key-value store.
    async fn prefetch(&self, hint: &str) -> Result<()> {
        let hint = Hint::parse(hint)?;
        let (hint_type, hint_data) = hint.split();
        trace!(target: "fetcher", "Fetching hint: {hint_type} {hint_data}");

        match hint_type {
            HintType::L1BlockHeader => {
                // Validate the hint data length.
                if hint_data.len() != 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                // Fetch the raw header from the L1 chain provider.
                let hash: B256 = hint_data
                    .as_ref()
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to B256: {e}"))?;
                let raw_header: Bytes = self
                    .l1_provider
                    .client()
                    .request("debug_getRawHeader", [hash])
                    .await
                    .map_err(|e| anyhow!(e))?;

                // Acquire a lock on the key-value store and set the preimage.
                let mut kv_lock = self.kv_store.write().await;
                kv_lock.set(
                    PreimageKey::new(*hash, PreimageKeyType::Keccak256).into(),
                    raw_header.into(),
                )?;
            }
            HintType::L1Transactions => {
                // Validate the hint data length.
                if hint_data.len() != 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                // Fetch the block from the L1 chain provider and store the transactions within its
                // body in the key-value store.
                let hash: B256 = hint_data
                    .as_ref()
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to B256: {e}"))?;
                let Block { transactions, .. } = self
                    .l1_provider
                    .get_block_by_hash(hash, BlockTransactionsKind::Full)
                    .await
                    .map_err(|e| anyhow!("Failed to fetch block: {e}"))?
                    .ok_or(anyhow!("Block not found."))?;
                self.store_transactions(transactions).await?;
            }
            HintType::L1Receipts => {
                // Validate the hint data length.
                if hint_data.len() != 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                // Fetch the receipts from the L1 chain provider and store the receipts within the
                // key-value store.
                let hash: B256 = hint_data
                    .as_ref()
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to B256: {e}"))?;
                let raw_receipts: Vec<Bytes> = self
                    .l1_provider
                    .client()
                    .request("debug_getRawReceipts", [hash])
                    .await
                    .map_err(|e| anyhow!(e))?;
                self.store_trie_nodes(raw_receipts.as_slice()).await?;
            }
            HintType::L1Blob => {
                if hint_data.len() != 48 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                let hash_data_bytes: [u8; 32] = hint_data[0..32]
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to B256: {e}"))?;
                let index_data_bytes: [u8; 8] = hint_data[32..40]
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to u64: {e}"))?;
                let timestamp_data_bytes: [u8; 8] = hint_data[40..48]
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to u64: {e}"))?;

                let hash: B256 = hash_data_bytes.into();
                let index = u64::from_be_bytes(index_data_bytes);
                let timestamp = u64::from_be_bytes(timestamp_data_bytes);

                let partial_block_ref = BlockInfo { timestamp, ..Default::default() };
                let indexed_hash = IndexedBlobHash { index, hash };

                // Fetch the blob sidecar from the blob provider.
                let mut sidecars = self
                    .blob_provider
                    .fetch_filtered_sidecars(&partial_block_ref, &[indexed_hash])
                    .await
                    .map_err(|e| anyhow!("Failed to fetch blob sidecars: {e}"))?;
                if sidecars.len() != 1 {
                    anyhow::bail!("Expected 1 sidecar, got {}", sidecars.len());
                }
                let sidecar = sidecars.remove(0);

                // Acquire a lock on the key-value store and set the preimages.
                let mut kv_write_lock = self.kv_store.write().await;

                // Set the preimage for the blob commitment.
                kv_write_lock.set(
                    PreimageKey::new(*hash, PreimageKeyType::Sha256).into(),
                    sidecar.kzg_commitment.to_vec(),
                )?;

                // Write all the field elements to the key-value store. There should be 4096.
                // The preimage oracle key for each field element is the keccak256 hash of
                // `abi.encodePacked(sidecar.KZGCommitment, uint256(i))`
                let mut blob_key = [0u8; 80];
                blob_key[..48].copy_from_slice(sidecar.kzg_commitment.as_ref());
                for i in 0..FIELD_ELEMENTS_PER_BLOB {
                    blob_key[72..].copy_from_slice(i.to_be_bytes().as_ref());
                    let blob_key_hash = keccak256(blob_key.as_ref());

                    kv_write_lock.set(
                        PreimageKey::new(*blob_key_hash, PreimageKeyType::Keccak256).into(),
                        blob_key.into(),
                    )?;
                    kv_write_lock.set(
                        PreimageKey::new(*blob_key_hash, PreimageKeyType::Blob).into(),
                        sidecar.blob[(i as usize) << 5..(i as usize + 1) << 5].to_vec(),
                    )?;
                }

                // Write the KZG Proof as the 4096th element.
                blob_key[72..].copy_from_slice((FIELD_ELEMENTS_PER_BLOB).to_be_bytes().as_ref());
                let blob_key_hash = keccak256(blob_key.as_ref());

                kv_write_lock.set(
                    PreimageKey::new(*blob_key_hash, PreimageKeyType::Keccak256).into(),
                    blob_key.into(),
                )?;
                kv_write_lock.set(
                    PreimageKey::new(*blob_key_hash, PreimageKeyType::Blob).into(),
                    sidecar.kzg_proof.to_vec(),
                )?;
            }
            HintType::L1Precompile => {
                // Validate the hint data length.
                if hint_data.len() < 20 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                // Fetch the precompile address from the hint data.
                let precompile_address = Address::from_slice(&hint_data.as_ref()[..20]);
                let precompile_input = hint_data[20..].to_vec();
                let input_hash = keccak256(hint_data.as_ref());

                let result = crate::eth::execute(precompile_address, precompile_input).map_or_else(
                    |_| vec![0u8; 1],
                    |raw_res| {
                        let mut res = Vec::with_capacity(1 + raw_res.len());
                        res.push(0x01);
                        res.extend_from_slice(&raw_res);
                        res
                    },
                );

                // Acquire a lock on the key-value store and set the preimages.
                let mut kv_lock = self.kv_store.write().await;
                kv_lock.set(
                    PreimageKey::new(*input_hash, PreimageKeyType::Keccak256).into(),
                    hint_data.into(),
                )?;
                kv_lock.set(
                    PreimageKey::new(*input_hash, PreimageKeyType::Precompile).into(),
                    result,
                )?;
            }
            HintType::L2BlockHeader => {
                // Validate the hint data length.
                if hint_data.len() != 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                // Fetch the raw header from the L2 chain provider.
                let hash: B256 = hint_data
                    .as_ref()
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to B256: {e}"))?;
                let raw_header: Bytes = self
                    .l2_provider
                    .client()
                    .request("debug_getRawHeader", [hash])
                    .await
                    .map_err(|e| anyhow!(e))?;

                // Acquire a lock on the key-value store and set the preimage.
                let mut kv_lock = self.kv_store.write().await;
                kv_lock.set(
                    PreimageKey::new(*hash, PreimageKeyType::Keccak256).into(),
                    raw_header.into(),
                )?;
            }
            HintType::L2Transactions => {
                // Validate the hint data length.
                if hint_data.len() != 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                // Fetch the block from the L2 chain provider and store the transactions within its
                // body in the key-value store.
                let hash: B256 = hint_data
                    .as_ref()
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to B256: {e}"))?;
                let Block { transactions, .. } = self
                    .l2_provider
                    .get_block_by_hash(hash, BlockTransactionsKind::Hashes)
                    .await
                    .map_err(|e| anyhow!("Failed to fetch block: {e}"))?
                    .ok_or(anyhow!("Block not found."))?;

                match transactions {
                    BlockTransactions::Hashes(transactions) => {
                        let mut encoded_transactions = Vec::with_capacity(transactions.len());
                        for tx_hash in transactions {
                            let tx = self
                                .l2_provider
                                .client()
                                .request::<&[B256; 1], Bytes>("debug_getRawTransaction", &[tx_hash])
                                .await
                                .map_err(|e| anyhow!("Error fetching transaction: {e}"))?;
                            encoded_transactions.push(tx);
                        }

                        self.store_trie_nodes(encoded_transactions.as_slice()).await?;
                    }
                    _ => anyhow::bail!("Only BlockTransactions::Hashes are supported."),
                };
            }
            HintType::L2Code => {
                // geth hashdb scheme code hash key prefix
                const CODE_PREFIX: u8 = b'c';

                if hint_data.len() != 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                let hash: B256 = hint_data
                    .as_ref()
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to B256: {e}"))?;

                // Attempt to fetch the code from the L2 chain provider.
                let code_hash = [&[CODE_PREFIX], hash.as_slice()].concat();
                let code = self
                    .l2_provider
                    .client()
                    .request::<&[Bytes; 1], Bytes>("debug_dbGet", &[code_hash.into()])
                    .await;

                // Check if the first attempt to fetch the code failed. If it did, try fetching the
                // code hash preimage without the geth hashdb scheme prefix.
                let code = match code {
                    Ok(code) => code,
                    Err(_) => self
                        .l2_provider
                        .client()
                        .request::<&[B256; 1], Bytes>("debug_dbGet", &[hash])
                        .await
                        .map_err(|e| anyhow!("Error fetching code hash preimage: {e}"))?,
                };

                let mut kv_write_lock = self.kv_store.write().await;
                kv_write_lock
                    .set(PreimageKey::new(*hash, PreimageKeyType::Keccak256).into(), code.into())?;
            }
            HintType::StartingL2Output => {
                const OUTPUT_ROOT_VERSION: u8 = 0;
                const L2_TO_L1_MESSAGE_PASSER_ADDRESS: Address =
                    address!("4200000000000000000000000000000000000016");

                if hint_data.len() != 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                // Fetch the header for the L2 head block.
                let raw_header: Bytes = self
                    .l2_provider
                    .client()
                    .request("debug_getRawHeader", &[self.l2_head])
                    .await
                    .map_err(|e| anyhow!("Failed to fetch header RLP: {e}"))?;
                let header = Header::decode(&mut raw_header.as_ref())
                    .map_err(|e| anyhow!("Failed to decode header: {e}"))?;

                // Fetch the storage root for the L2 head block.
                let l2_to_l1_message_passer = self
                    .l2_provider
                    .get_proof(L2_TO_L1_MESSAGE_PASSER_ADDRESS, Default::default())
                    .block_id(BlockId::Hash(self.l2_head.into()))
                    .await
                    .map_err(|e| anyhow!("Failed to fetch account proof: {e}"))?;

                let mut raw_output = [0u8; 128];
                raw_output[31] = OUTPUT_ROOT_VERSION;
                raw_output[32..64].copy_from_slice(header.state_root.as_ref());
                raw_output[64..96].copy_from_slice(l2_to_l1_message_passer.storage_hash.as_ref());
                raw_output[96..128].copy_from_slice(self.l2_head.as_ref());
                let output_root = keccak256(raw_output);

                if output_root.as_slice() != hint_data.as_ref() {
                    anyhow::bail!("Output root does not match L2 head.");
                }

                let mut kv_write_lock = self.kv_store.write().await;
                kv_write_lock.set(
                    PreimageKey::new(*output_root, PreimageKeyType::Keccak256).into(),
                    raw_output.into(),
                )?;
            }
            HintType::L2StateNode => {
                if hint_data.len() != 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                let hash: B256 = hint_data
                    .as_ref()
                    .try_into()
                    .map_err(|e| anyhow!("Failed to convert bytes to B256: {e}"))?;

                // Fetch the preimage from the L2 chain provider.
                let preimage: Bytes = self
                    .l2_provider
                    .client()
                    .request("debug_dbGet", &[hash])
                    .await
                    .map_err(|e| anyhow!("Failed to fetch preimage: {e}"))?;

                let mut kv_write_lock = self.kv_store.write().await;
                kv_write_lock.set(
                    PreimageKey::new(*hash, PreimageKeyType::Keccak256).into(),
                    preimage.into(),
                )?;
            }
            HintType::L2AccountProof => {
                if hint_data.len() != 8 + 20 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                let block_number = u64::from_be_bytes(
                    hint_data.as_ref()[..8]
                        .try_into()
                        .map_err(|e| anyhow!("Error converting hint data to u64: {e}"))?,
                );
                let address = Address::from_slice(&hint_data.as_ref()[8..28]);

                let proof_response = self
                    .l2_provider
                    .get_proof(address, Default::default())
                    .block_id(BlockId::Number(BlockNumberOrTag::Number(block_number)))
                    .await
                    .map_err(|e| anyhow!("Failed to fetch account proof: {e}"))?;

                let mut kv_write_lock = self.kv_store.write().await;

                // Write the account proof nodes to the key-value store.
                proof_response.account_proof.into_iter().try_for_each(|node| {
                    let node_hash = keccak256(node.as_ref());
                    let key = PreimageKey::new(*node_hash, PreimageKeyType::Keccak256);
                    kv_write_lock.set(key.into(), node.into())?;
                    Ok::<(), anyhow::Error>(())
                })?;
            }
            HintType::L2AccountStorageProof => {
                if hint_data.len() != 8 + 20 + 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }

                let block_number = u64::from_be_bytes(
                    hint_data.as_ref()[..8]
                        .try_into()
                        .map_err(|e| anyhow!("Error converting hint data to u64: {e}"))?,
                );
                let address = Address::from_slice(&hint_data.as_ref()[8..28]);
                let slot = B256::from_slice(&hint_data.as_ref()[28..]);

                let mut proof_response = self
                    .l2_provider
                    .get_proof(address, vec![slot])
                    .block_id(BlockId::Number(BlockNumberOrTag::Number(block_number)))
                    .await
                    .map_err(|e| anyhow!("Failed to fetch account proof: {e}"))?;

                let mut kv_write_lock = self.kv_store.write().await;

                // Write the account proof nodes to the key-value store.
                proof_response.account_proof.into_iter().try_for_each(|node| {
                    let node_hash = keccak256(node.as_ref());
                    let key = PreimageKey::new(*node_hash, PreimageKeyType::Keccak256);
                    kv_write_lock.set(key.into(), node.into())?;
                    Ok::<(), anyhow::Error>(())
                })?;

                // Write the storage proof nodes to the key-value store.
                let storage_proof = proof_response.storage_proof.remove(0);
                storage_proof.proof.into_iter().try_for_each(|node| {
                    let node_hash = keccak256(node.as_ref());
                    let key = PreimageKey::new(*node_hash, PreimageKeyType::Keccak256);
                    kv_write_lock.set(key.into(), node.into())?;
                    Ok::<(), anyhow::Error>(())
                })?;
            }
            HintType::L2PayloadWitness => {
                if hint_data.len() < 32 {
                    anyhow::bail!("Invalid hint data length: {}", hint_data.len());
                }
                let parent_block_hash = B256::from_slice(&hint_data.as_ref()[..32]);
                let payload_attributes: OpPayloadAttributes =
                    serde_json::from_slice(&hint_data[32..])?;

                let execute_payload_response: ExecutionWitness = self
                    .l2_provider
                    .client()
                    .request::<(B256, OpPayloadAttributes), ExecutionWitness>(
                        "debug_executePayload",
                        (parent_block_hash, payload_attributes),
                    )
                    .await
                    .map_err(|e| anyhow!("Failed to fetch preimage: {e}"))?;

                let mut merged = HashMap::<B256, Bytes>::default();
                merged.extend(execute_payload_response.state);
                merged.extend(execute_payload_response.codes);
                merged.extend(execute_payload_response.keys);

                let mut kv_write_lock = self.kv_store.write().await;
                for (hash, preimage) in merged.into_iter() {
                    let computed_hash = keccak256(preimage.as_ref());
                    assert_eq!(computed_hash, hash, "Preimage hash does not match expected hash");

                    let key = PreimageKey::new(*hash, PreimageKeyType::Keccak256);
                    kv_write_lock.set(key.into(), preimage.into())?;
                }
            }
        }

        Ok(())
    }

    /// Stores a list of [BlockTransactions] in the key-value store.
    async fn store_transactions(&self, transactions: BlockTransactions<Transaction>) -> Result<()> {
        match transactions {
            BlockTransactions::Full(transactions) => {
                let encoded_transactions = transactions
                    .into_iter()
                    .map(|tx| {
                        let envelope: TxEnvelope = tx.into();

                        Ok::<_, anyhow::Error>(envelope.encoded_2718())
                    })
                    .collect::<Result<Vec<_>>>()?;

                self.store_trie_nodes(encoded_transactions.as_slice()).await
            }
            _ => anyhow::bail!("Only BlockTransactions::Full are supported."),
        }
    }

    /// Stores intermediate trie nodes in the key-value store. Assumes that all nodes passed are
    /// raw, RLP encoded trie nodes.
    async fn store_trie_nodes<T: AsRef<[u8]>>(&self, nodes: &[T]) -> Result<()> {
        let mut kv_write_lock = self.kv_store.write().await;

        // If the list of nodes is empty, store the empty root hash and exit early.
        // The `HashBuilder` will not push the preimage of the empty root hash to the
        // `ProofRetainer` in the event that there are no leaves inserted.
        if nodes.is_empty() {
            let empty_key = PreimageKey::new(*EMPTY_ROOT_HASH, PreimageKeyType::Keccak256);
            return kv_write_lock.set(empty_key.into(), [EMPTY_STRING_CODE].into());
        }

        let mut hb = kona_mpt::ordered_trie_with_encoder(nodes, |node, buf| {
            buf.put_slice(node.as_ref());
        });
        hb.root();
        let intermediates = hb.take_proof_nodes().into_inner();

        for (_, value) in intermediates.into_iter() {
            let value_hash = keccak256(value.as_ref());
            let key = PreimageKey::new(*value_hash, PreimageKeyType::Keccak256);

            kv_write_lock.set(key.into(), value.into())?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl<KV> HintRouter for SingleChainFetcher<KV>
where
    KV: KeyValueStore + Send + Sync + ?Sized,
{
    /// Set the last hint to be received.
    async fn route_hint(&self, hint: String) -> PreimageOracleResult<()> {
        trace!(target: "fetcher", "Received hint: {hint}");
        let mut hint_lock = self.last_hint.write().await;
        hint_lock.replace(hint);
        Ok(())
    }
}

#[async_trait::async_trait]
impl<KV> PreimageFetcher for SingleChainFetcher<KV>
where
    KV: KeyValueStore + Send + Sync + ?Sized,
{
    /// Get the preimage for the given key.
    async fn get_preimage(&self, key: PreimageKey) -> PreimageOracleResult<Vec<u8>> {
        trace!(target: "fetcher", "Pre-image requested. Key: {key}");

        // Acquire a read lock on the key-value store.
        let kv_lock = self.kv_store.read().await;
        let mut preimage = kv_lock.get(key.into());

        // Drop the read lock before beginning the retry loop.
        drop(kv_lock);

        // Use a loop to keep retrying the prefetch as long as the key is not found
        while preimage.is_none() {
            match self.last_hint.read().await.as_ref() {
                None => continue,
                Some(hint) => {
                    if let Err(e) = self.prefetch(hint).await {
                        error!(target: "fetcher", "Failed to prefetch hint: {e}");
                        warn!(target: "fetcher", "Retrying hint fetch: {hint}");
                        continue;
                    }

                    let kv_lock = self.kv_store.read().await;
                    preimage = kv_lock.get(key.into());
                }
            }
        }

        preimage.ok_or(PreimageOracleError::KeyNotFound)
    }
}
