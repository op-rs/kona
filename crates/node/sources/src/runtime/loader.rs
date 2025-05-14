//! Contains the [`RuntimeLoader`] implementation.

use crate::{RuntimeConfig, RuntimeLoaderError};
use alloy_primitives::{Address, B256, b256};
use alloy_provider::Provider;
use kona_derive::traits::ChainProvider;
use kona_genesis::RollupConfig;
use kona_protocol::BlockInfo;
use kona_providers_alloy::AlloyChainProvider;
use lru::LruCache;
use op_alloy_rpc_types_engine::ProtocolVersion;
use std::{num::NonZeroUsize, sync::Arc};
use url::Url;

/// The default cache size for the [`RuntimeLoader`].
const DEFAULT_CACHE_SIZE: usize = 100;

/// The storage slot that the unsafe block signer address is stored at.
/// Computed as: `bytes32(uint256(keccak256("systemconfig.unsafeblocksigner")) - 1)`
const UNSAFE_BLOCK_SIGNER_ADDRESS_STORAGE_SLOT: B256 =
    b256!("0x65a7ed542fb37fe237fdfbdd70b31598523fe5b32879e307bae27a0bd9581c08");

/// The storage slot that the required protocol version is stored at.
/// Computed as: `bytes32(uint256(keccak256("protocolversion.required")) - 1)`
const REQUIRED_PROTOCOL_VERSION_STORAGE_SLOT: B256 =
    b256!("0x4aaefe95bd84fd3f32700cf3b7566bc944b73138e41958b5785826df2aecace0");

/// The storage slot that the recommended protocol version is stored at.
/// Computed as: `bytes32(uint256(keccak256("protocolversion.recommended")) - 1)`
const RECOMMENDED_PROTOCOL_VERSION_STORAGE_SLOT: B256 =
    b256!("0xe314dfc40f0025322aacc0ba8ef420b62fb3b702cf01e0cdf3d829117ac2ff1a");

/// The runtime loader.
#[derive(Debug, Clone)]
pub struct RuntimeLoader {
    /// The L1 Client
    pub provider: AlloyChainProvider,
    /// The rollup config.
    pub config: Arc<RollupConfig>,
    /// Caches the previously loaded runtime config.
    runtime: RuntimeConfig,
    /// Cache mapping [`BlockInfo`] to the [`RuntimeConfig`].
    ///
    /// If the block hash for the given block info is a mismatch, the runtime config
    /// will be reloaded.
    pub cache: LruCache<BlockInfo, RuntimeConfig>,
}

impl RuntimeLoader {
    /// Constructs a new [`RuntimeLoader`] with the given provider [`Url`].
    pub fn new(l1_eth_rpc: Url, config: Arc<RollupConfig>) -> Self {
        let provider = AlloyChainProvider::new_http(l1_eth_rpc, DEFAULT_CACHE_SIZE);
        Self {
            provider,
            config,
            cache: LruCache::new(NonZeroUsize::new(DEFAULT_CACHE_SIZE).unwrap()),
            runtime: RuntimeConfig {
                unsafe_block_signer_address: Address::ZERO,
                required_protocol_version: ProtocolVersion::V0(Default::default()),
                recommended_protocol_version: ProtocolVersion::V0(Default::default()),
            },
        }
    }

    /// Loads the [`RuntimeConfig`] for the latest block.
    pub async fn load_latest(&mut self) -> Result<RuntimeConfig, RuntimeLoaderError> {
        let latest_block_num = self.provider.latest_block_number().await?;
        let block_info = self.provider.block_info_by_number(latest_block_num).await?;
        self.load(block_info).await
    }

    /// Loads the [`RuntimeConfig`] for the given [`BlockInfo`].
    pub async fn load(
        &mut self,
        block_info: BlockInfo,
    ) -> Result<RuntimeConfig, RuntimeLoaderError> {
        // Check if the runtime config is already cached.
        if let Some(config) = self.cache.get(&block_info) {
            // Only use the cached config if the block hash matches.
            let block = self.provider.inner.get_block(block_info.hash.into()).await?;
            if block.is_some_and(|block| block.header.hash == block_info.hash) {
                debug!(target: "runtime_loader", "Using cached runtime config");
                return Ok(*config);
            }
        }

        // Fetch the unsafe block signer address from the system config.
        let unsafe_block_signer_address = self
            .provider
            .inner
            .get_storage_at(
                self.config.l1_system_config_address,
                UNSAFE_BLOCK_SIGNER_ADDRESS_STORAGE_SLOT.into(),
            )
            .hash(block_info.hash)
            .await?;

        // Convert the unsafe block signer address to the correct type.
        let unsafe_block_signer_address = alloy_primitives::Address::from_slice(
            &unsafe_block_signer_address.to_be_bytes_vec()[12..],
        );
        debug!(target: "runtime_loader", "Unsafe block signer address: {:#x}", unsafe_block_signer_address);

        // If the protocol versions address is not set, return the default config.
        let mut required_protocol_version = self.runtime.required_protocol_version;
        let mut recommended_protocol_version = self.runtime.recommended_protocol_version;

        // Fetch the required protocol version from the system config.
        if self.config.protocol_versions_address != Address::ZERO {
            let required = self
                .provider
                .inner
                .get_storage_at(
                    self.config.protocol_versions_address,
                    REQUIRED_PROTOCOL_VERSION_STORAGE_SLOT.into(),
                )
                .hash(block_info.hash)
                .await?;
            required_protocol_version = ProtocolVersion::decode(required.into())?;
            debug!(target: "runtime_loader", "Required protocol version: {:?}", required_protocol_version);

            let recommended = self
                .provider
                .inner
                .get_storage_at(
                    self.config.protocol_versions_address,
                    RECOMMENDED_PROTOCOL_VERSION_STORAGE_SLOT.into(),
                )
                .hash(block_info.hash)
                .await?;
            recommended_protocol_version = ProtocolVersion::decode(recommended.into())?;
            debug!(target: "runtime_loader", "Recommended protocol version: {:?}", recommended_protocol_version);
        } else {
            warn!(target: "runtime_loader", "Protocol versions address is not set in Rollup Config.");
            warn!(target: "runtime_loader", "Using default protocol version: {:?}", required_protocol_version);
        }

        // Metrics
        kona_macros::record!(
            histogram,
            crate::Metrics::RUNTIME_LOADER,
            "unsafe_block_signer",
            format!("{:#x}", unsafe_block_signer_address),
            1
        );
        kona_macros::record!(
            histogram,
            crate::Metrics::RUNTIME_LOADER,
            "recommended_version",
            recommended_protocol_version.to_string(),
            1
        );
        kona_macros::record!(
            histogram,
            crate::Metrics::RUNTIME_LOADER,
            "required_version",
            required_protocol_version.to_string(),
            1
        );

        // Construct the runtime config.
        let runtime_config = RuntimeConfig {
            unsafe_block_signer_address,
            required_protocol_version,
            recommended_protocol_version,
        };
        debug!(target: "runtime_loader", "{}", runtime_config);
        self.runtime = runtime_config;

        // Cache the runtime config.
        self.cache.put(block_info, runtime_config);

        Ok(runtime_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;
    use op_alloy_rpc_types_engine::ProtocolVersionFormatV0;

    const RPC_URL: &str = "https://docs-demo.quiknode.pro/";

    #[tokio::test]
    async fn test_online_runtime_loader() {
        kona_cli::init_test_tracing();

        // Load the OP Mainnet config.
        let chain_id = kona_genesis::OP_MAINNET_CHAIN_ID;
        let config =
            kona_registry::ROLLUP_CONFIGS.get(&chain_id).expect("Invalid chain ID").clone();

        // Construct the runtime loader.
        let config = Arc::new(config);
        let url = Url::parse(RPC_URL).unwrap();
        let mut loader = RuntimeLoader::new(url.clone(), config);

        // Load the runtime config.
        let version = ProtocolVersionFormatV0 { major: 9, ..Default::default() };
        let expected = RuntimeConfig {
            unsafe_block_signer_address: address!("aaaa45d9549eda09e70937013520214382ffc4a2"),
            required_protocol_version: ProtocolVersion::V0(version),
            recommended_protocol_version: ProtocolVersion::V0(version),
        };
        let runtime_config = loader.load_latest().await.unwrap();
        assert_eq!(runtime_config, expected);
    }
}
