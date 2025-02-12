//! Test utilities for the executor.

#![allow(missing_docs, unused)]

use crate::{constants::FEE_RECIPIENT, StatelessL2BlockExecutor, TrieDBProvider};
use alloy_consensus::Header;
use alloy_primitives::{Bytes, Sealable, B256};
use alloy_provider::{
    network::primitives::{BlockTransactions, BlockTransactionsKind},
    Provider, RootProvider,
};
use alloy_rlp::Decodable;
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_engine::PayloadAttributes;
use alloy_transport_http::{Client, Http};
use kona_host::{DiskKeyValueStore, KeyValueStore};
use kona_mpt::{NoopTrieHinter, TrieNode, TrieProvider};
use maili_genesis::RollupConfig;
use maili_registry::ROLLUP_CONFIGS;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use serde::{Deserialize, Serialize};
use std::{env::temp_dir, path::PathBuf, sync::Arc};
use tokio::{fs, runtime::Handle, sync::Mutex};

#[derive(Debug, thiserror::Error)]
pub(crate) enum TestTrieNodeProviderError {
    #[error("Preimage not found")]
    PreimageNotFound,
    #[error("Failed to decode RLP: {0}")]
    Rlp(alloy_rlp::Error),
    #[error("Failed to write back to key value store")]
    KVStore,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ExecutorTestFixture {
    /// The rollup configuration for the executing chain.
    pub(crate) rollup_config: RollupConfig,
    /// The parent block header.
    pub(crate) parent_header: Header,
    /// The executing payload attributes.
    pub(crate) executing_payload: OpPayloadAttributes,
    /// The expected block hash
    pub(crate) expected_block_hash: B256,
}

#[derive(Debug)]
pub(crate) struct ExecutorTestFixtureCreator {
    /// The RPC provider for the L2 execution layer.
    pub(crate) provider: RootProvider,
    /// The block number to create the test fixture for.
    pub(crate) block_number: u64,
    /// The key value store for the test fixture.
    pub(crate) kv_store: Arc<Mutex<DiskKeyValueStore>>,
    /// The data directory for the test fixture.
    pub(crate) data_dir: PathBuf,
}

impl ExecutorTestFixtureCreator {
    pub(crate) fn new(
        provider_url: &str,
        block_number: u64,
        base_fixture_directory: PathBuf,
    ) -> Self {
        let base = base_fixture_directory.join(format!("block-{}", block_number));

        let url = provider_url.parse().expect("Invalid provider URL");
        let http = Http::<Client>::new(url);
        let provider = RootProvider::new(RpcClient::new(http, false));

        Self {
            provider,
            block_number,
            kv_store: Arc::new(Mutex::new(DiskKeyValueStore::new(base.join("kv")))),
            data_dir: base,
        }
    }
}

impl ExecutorTestFixtureCreator {
    /// Create a static test fixture with the configuration provided.
    pub(crate) async fn create_static_fixture(self) {
        let chain_id = self.provider.get_chain_id().await.expect("Failed to get chain ID");
        let rollup_config = ROLLUP_CONFIGS.get(&chain_id).expect("Rollup config not found");

        let executing_block = self
            .provider
            .get_block_by_number(self.block_number.into(), BlockTransactionsKind::Hashes)
            .await
            .expect("Failed to get parent block")
            .expect("Block not found");
        let parent_block = self
            .provider
            .get_block_by_number((self.block_number - 1).into(), BlockTransactionsKind::Hashes)
            .await
            .expect("Failed to get parent block")
            .expect("Block not found");

        let executing_header = executing_block.header;
        let parent_header = parent_block.header.inner.seal_slow();

        let encoded_executing_transactions = match executing_block.transactions {
            BlockTransactions::Hashes(transactions) => {
                let mut encoded_transactions = Vec::with_capacity(transactions.len());
                for tx_hash in transactions {
                    let tx = self
                        .provider
                        .client()
                        .request::<&[B256; 1], Bytes>("debug_getRawTransaction", &[tx_hash])
                        .await
                        .expect("Block not found");
                    encoded_transactions.push(tx);
                }
                encoded_transactions
            }
            _ => panic!("Only BlockTransactions::Hashes are supported."),
        };

        let payload_attrs = OpPayloadAttributes {
            payload_attributes: PayloadAttributes {
                timestamp: executing_header.timestamp,
                parent_beacon_block_root: parent_header.parent_beacon_block_root,
                prev_randao: parent_header.mix_hash,
                withdrawals: Default::default(),
                suggested_fee_recipient: FEE_RECIPIENT,
            },
            gas_limit: Some(executing_header.gas_limit),
            transactions: Some(encoded_executing_transactions),
            no_tx_pool: None,
            eip_1559_params: rollup_config.is_holocene_active(executing_header.timestamp).then(
                || {
                    executing_header.extra_data[1..]
                        .try_into()
                        .expect("Invalid header format for Holocene")
                },
            ),
        };

        let fixture_path = self.data_dir.join("fixture.json");
        let fixture = ExecutorTestFixture {
            rollup_config: rollup_config.clone(),
            parent_header: parent_header.inner().clone(),
            executing_payload: payload_attrs.clone(),
            expected_block_hash: executing_header.hash_slow(),
        };

        let mut executor = StatelessL2BlockExecutor::builder(rollup_config, self, NoopTrieHinter)
            .with_parent_header(parent_header)
            .build();
        let exec_artifacts =
            executor.execute_payload(payload_attrs).expect("Failed to execute block").clone();

        assert_eq!(
            exec_artifacts.block_header.inner(),
            &executing_header.inner,
            "Produced header does not match the expected header"
        );
        fs::write(fixture_path.as_path(), serde_json::to_vec(&fixture).unwrap()).await.unwrap();

        // Tar the fixture.
        let data_dir = fixture_path.parent().unwrap();
        tokio::process::Command::new("tar")
            .arg("-czf")
            .arg(data_dir.with_extension("tar.gz").file_name().unwrap())
            .arg(data_dir.file_name().unwrap())
            .current_dir(data_dir.parent().unwrap())
            .output()
            .await
            .expect("Failed to tar fixture");

        // Remove the leftover directory.
        fs::remove_dir_all(data_dir).await.expect("Failed to remove temporary directory");
    }
}

impl TrieProvider for ExecutorTestFixtureCreator {
    type Error = TestTrieNodeProviderError;

    fn trie_node_by_hash(&self, key: B256) -> Result<TrieNode, Self::Error> {
        // Fetch the preimage from the L2 chain provider.
        let preimage: Bytes = tokio::task::block_in_place(move || {
            Handle::current().block_on(async {
                let preimage: Bytes = self
                    .provider
                    .client()
                    .request("debug_dbGet", &[key])
                    .await
                    .map_err(|_| TestTrieNodeProviderError::PreimageNotFound)?;

                self.kv_store
                    .lock()
                    .await
                    .set(key, preimage.clone().into())
                    .map_err(|_| TestTrieNodeProviderError::KVStore)?;

                Ok(preimage)
            })
        })?;

        // Decode the preimage into a trie node.
        TrieNode::decode(&mut preimage.as_ref()).map_err(TestTrieNodeProviderError::Rlp)
    }
}

impl TrieDBProvider for ExecutorTestFixtureCreator {
    fn bytecode_by_hash(&self, hash: B256) -> Result<Bytes, Self::Error> {
        // geth hashdb scheme code hash key prefix
        const CODE_PREFIX: u8 = b'c';

        // Fetch the preimage from the L2 chain provider.
        let preimage: Bytes = tokio::task::block_in_place(move || {
            Handle::current().block_on(async {
                // Attempt to fetch the code from the L2 chain provider.
                let code_hash = [&[CODE_PREFIX], hash.as_slice()].concat();
                let code = self
                    .provider
                    .client()
                    .request::<&[Bytes; 1], Bytes>("debug_dbGet", &[code_hash.into()])
                    .await;

                // Check if the first attempt to fetch the code failed. If it did, try fetching the
                // code hash preimage without the geth hashdb scheme prefix.
                let code = match code {
                    Ok(code) => code,
                    Err(_) => self
                        .provider
                        .client()
                        .request::<&[B256; 1], Bytes>("debug_dbGet", &[hash])
                        .await
                        .map_err(|_| TestTrieNodeProviderError::PreimageNotFound)?,
                };

                self.kv_store
                    .lock()
                    .await
                    .set(hash, code.clone().into())
                    .map_err(|_| TestTrieNodeProviderError::KVStore)?;

                Ok(code)
            })
        })?;

        Ok(preimage)
    }

    fn header_by_hash(&self, hash: B256) -> Result<Header, Self::Error> {
        let encoded_header: Bytes = tokio::task::block_in_place(move || {
            Handle::current().block_on(async {
                let preimage: Bytes = self
                    .provider
                    .client()
                    .request("debug_getRawHeader", &[hash])
                    .await
                    .map_err(|_| TestTrieNodeProviderError::PreimageNotFound)?;

                self.kv_store
                    .lock()
                    .await
                    .set(hash, preimage.clone().into())
                    .map_err(|_| TestTrieNodeProviderError::KVStore)?;

                Ok(preimage)
            })
        })?;

        // Decode the Header.
        Header::decode(&mut encoded_header.as_ref()).map_err(TestTrieNodeProviderError::Rlp)
    }
}

struct DiskTrieNodeProvider {
    kv_store: DiskKeyValueStore,
}

impl DiskTrieNodeProvider {
    pub(crate) const fn new(kv_store: DiskKeyValueStore) -> Self {
        Self { kv_store }
    }
}

impl TrieProvider for DiskTrieNodeProvider {
    type Error = TestTrieNodeProviderError;

    fn trie_node_by_hash(&self, key: B256) -> Result<TrieNode, Self::Error> {
        TrieNode::decode(
            &mut self
                .kv_store
                .get(key)
                .ok_or(TestTrieNodeProviderError::PreimageNotFound)?
                .as_slice(),
        )
        .map_err(TestTrieNodeProviderError::Rlp)
    }
}

impl TrieDBProvider for DiskTrieNodeProvider {
    fn bytecode_by_hash(&self, code_hash: B256) -> Result<Bytes, Self::Error> {
        self.kv_store
            .get(code_hash)
            .ok_or(TestTrieNodeProviderError::PreimageNotFound)
            .map(Bytes::from)
    }

    fn header_by_hash(&self, hash: B256) -> Result<Header, Self::Error> {
        Header::decode(
            &mut self
                .kv_store
                .get(hash)
                .ok_or(TestTrieNodeProviderError::PreimageNotFound)?
                .as_slice(),
        )
        .map_err(TestTrieNodeProviderError::Rlp)
    }
}

/// Executes a [ExecutorTestFixture] stored at the passed `fixture_path` and asserts that the
/// produced block hash matches the expected block hash.
pub(crate) async fn run_test_fixture(fixture_path: PathBuf) {
    // First, untar the fixture.
    let mut fixture_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let untar = tokio::process::Command::new("tar")
        .arg("-xvf")
        .arg(fixture_path.as_path())
        .arg("-C")
        .arg(fixture_dir.path())
        .arg("--strip-components=1")
        .output()
        .await
        .expect("Failed to untar fixture");

    let kv_store = DiskKeyValueStore::new(fixture_dir.path().join("kv"));
    let provider = DiskTrieNodeProvider::new(kv_store);
    let fixture: ExecutorTestFixture =
        serde_json::from_slice(&fs::read(fixture_dir.path().join("fixture.json")).await.unwrap())
            .expect("Failed to deserialize fixture");

    let mut executor =
        StatelessL2BlockExecutor::builder(&fixture.rollup_config, provider, NoopTrieHinter)
            .with_parent_header(fixture.parent_header.seal_slow())
            .build();

    let exec_artifacts = executor.execute_payload(fixture.executing_payload).unwrap();

    assert_eq!(
        exec_artifacts.block_header.hash(),
        fixture.expected_block_hash,
        "Produced header does not match the expected header"
    );
}
