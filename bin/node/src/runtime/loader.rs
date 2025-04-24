//! Contains the [`RuntimeLoader`] implementation.

use super::{RuntimeConfig, RuntimeLoaderError};
use alloy_primitives::{Address, B256, b256};
use alloy_provider::{Provider, RootProvider};
use kona_genesis::RollupConfig;
use kona_protocol::BlockInfo;
use kona_rpc::ProtocolVersion;
use lru::LruCache;
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
    pub provider: RootProvider,
    /// The rollup config.
    pub config: Arc<RollupConfig>,
    /// Cache mapping [`BlockInfo`] to the [`RuntimeConfig`].
    ///
    /// If the block hash for the given block info is a mismatch, the runtime config
    /// will be reloaded.
    pub cache: LruCache<BlockInfo, RuntimeConfig>,
}

impl RuntimeLoader {
    /// Constructs a new [`RuntimeLoader`] with the given provider [`Url`].
    pub fn new(l1_eth_rpc: Url, config: Arc<RollupConfig>) -> Self {
        let provider = RootProvider::new_http(l1_eth_rpc);
        Self {
            provider,
            config,
            cache: LruCache::new(NonZeroUsize::new(DEFAULT_CACHE_SIZE).unwrap()),
        }
    }

    /// Loads the [`RuntimeConfig`] for the given [`BlockInfo`].
    pub async fn load(
        &mut self,
        block_info: BlockInfo,
    ) -> Result<RuntimeConfig, RuntimeLoaderError> {
        // Check if the runtime config is already cached.
        if let Some(config) = self.cache.get(&block_info) {
            // Only use the cached config if the block hash matches.
            let block = self.provider.get_block(block_info.hash.into()).await?;
            if block.is_some_and(|block| block.header.hash == block_info.hash) {
                return Ok(config.clone());
            }
        }

        // Fetch the unsafe block signer address from the system config.
        let unsafe_block_signer_address = self
            .provider
            .get_storage_at(
                self.config.l1_system_config_address,
                UNSAFE_BLOCK_SIGNER_ADDRESS_STORAGE_SLOT.into(),
            )
            .hash(block_info.hash)
            .await?;

        // Convert the unsafe block signer address to the correct type.
        let unsafe_block_signer_address =
            alloy_primitives::Address::from(unsafe_block_signer_address);

        let mut required_protocol_version = ProtocolVersion::V0(Default::default());
        let mut recommended_protocol_version = ProtocolVersion::V0(Default::default());

        // Fetch the required protocol version from the system config.
        if self.config.protocol_versions_address != Address::ZERO {
            required_protocol_version = self
                .provider
                .get_storage_at(
                    self.config.protocol_versions_address,
                    REQUIRED_PROTOCOL_VERSION_STORAGE_SLOT.into(),
                )
                .hash(block_info.hash)
                .await?;

            recommended_protocol_version = self
                .provider
                .get_storage_at(
                    self.config.protocol_versions_address,
                    RECOMMENDED_PROTOCOL_VERSION_STORAGE_SLOT.into(),
                )
                .hash(block_info.hash)
                .await?;
        }

        // Construct the runtime config.
        let runtime_config = RuntimeConfig {
            unsafe_block_signer_address: alloy_primitives::Address::ZERO,
            required_protocol_version: ProtocolVersion::V0(Default::default()),
            recommended_protocol_version: ProtocolVersion::V0(Default::default()),
        };

        // Cache the runtime config.
        self.cache.put(block_info, runtime_config.clone());

        Ok(runtime_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_loader() {
        let config = Arc::new(RollupConfig::default());
        let url = Url::parse("http://localhost:8545").unwrap();
        let mut loader = RuntimeLoader::new(url, config);

        let block_info = BlockInfo { number: 1, ..Default::default() };

        let runtime_config = loader.load(block_info).await.unwrap();
        assert_eq!(runtime_config.unsafe_block_signer_address, alloy_primitives::Address::ZERO);
    }
}
