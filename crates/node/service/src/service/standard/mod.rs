//! Standard implementation of the [RollupNode] service, using the governance approved
//! OP Stack configuration of components.
//!
//! See: <https://specs.optimism.io/protocol/rollup-node.html>

use super::{NodeMode, RollupNodeService, SequencerNodeService, ValidatorNodeService};
use crate::{L1WatcherRpc, L2ForkchoiceState, SyncStartError, find_starting_forkchoice};
use alloy_provider::RootProvider;
use async_trait::async_trait;
use libp2p::identity::Keypair;
use op_alloy_network::Optimism;
use std::{net::SocketAddr, sync::Arc};
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tracing::info;

use kona_derive::{errors::PipelineErrorKind, traits::ChainProvider};
use kona_genesis::RollupConfig;
use kona_p2p::{Discv5BuilderError, GossipDriverBuilderError, Network, NetworkBuilderError};
use kona_protocol::BlockInfo;
use kona_providers_alloy::{
    AlloyChainProvider, AlloyChainProviderError, AlloyL2ChainProvider, OnlineBeaconClient,
    OnlineBlobProvider, OnlinePipeline,
};
use kona_rpc::RpcConfig;

mod builder;
pub use builder::RollupNodeBuilder;

/// The size of the cache used in the derivation pipeline's providers.
const DERIVATION_PROVIDER_CACHE_SIZE: usize = 1024;

/// The standard implementation of the [RollupNode] service, using the governance approved OP Stack
/// configuration of components.
#[derive(Debug)]
pub struct RollupNode {
    /// The rollup configuration.
    pub(crate) config: Arc<RollupConfig>,
    /// The mode of operation for the node.
    pub(crate) mode: NodeMode,
    /// The L1 EL provider.
    pub(crate) l1_provider: RootProvider,
    /// The L1 beacon API.
    pub(crate) l1_beacon: OnlineBeaconClient,
    /// The L2 EL provider.
    pub(crate) l2_provider: RootProvider<Optimism>,
    /// The L2 engine.
    ///
    /// TODO: Place L2 Engine API client here once it's ready.
    pub(crate) _l2_engine: (),
    /// Whether p2p networking is entirely disabled.
    pub(crate) network_disabled: bool,
    /// The keypair for the network driver.
    pub(crate) keypair: Option<Keypair>,
    /// The discovery socket address.
    pub(crate) discovery_address: Option<SocketAddr>,
    /// The gossip socket address.
    pub(crate) gossip_address: Option<SocketAddr>,
    /// The RPC configuration.
    #[allow(unused)]
    pub(crate) rpc_config: Option<RpcConfig>,
}

impl RollupNode {
    /// Creates a new [RollupNodeBuilder], instantiated with the given [RollupConfig].
    pub fn builder(config: RollupConfig) -> RollupNodeBuilder {
        RollupNodeBuilder::new(config)
    }
}

#[async_trait]
impl RollupNodeService for RollupNode {
    fn mode(&self) -> NodeMode {
        self.mode
    }
}

#[async_trait]
impl ValidatorNodeService for RollupNode {
    type DataAvailabilityWatcher = L1WatcherRpc;
    type DerivationPipeline = OnlinePipeline;
    type Error = RollupNodeError;

    fn config(&self) -> &RollupConfig {
        &self.config
    }

    fn new_da_watcher(
        &self,
        new_da_tx: UnboundedSender<BlockInfo>,
        cancellation: CancellationToken,
    ) -> Self::DataAvailabilityWatcher {
        L1WatcherRpc::new(self.l1_provider.clone(), new_da_tx, cancellation)
    }

    async fn init_network(&self) -> Result<Option<Network>, Self::Error> {
        if self.network_disabled {
            return Ok(None);
        }
        let gossip_addr = self.gossip_address.ok_or_else(|| {
            RollupNodeError::Network(NetworkBuilderError::GossipDriverBuilder(
                GossipDriverBuilderError::GossipAddrNotSet,
            ))
        })?;
        let disc_addr = self.discovery_address.ok_or_else(|| {
            RollupNodeError::Network(NetworkBuilderError::DiscoveryDriverBuilder(
                Discv5BuilderError::ListenConfigNotSet,
            ))
        })?;
        let keypair = self.keypair.clone().unwrap_or_else(Keypair::generate_secp256k1);

        // TODO: grab the unsafe block signer from the config.
        // Only in chain config and not rollup config...
        let signer = alloy_primitives::Address::default();
        let chain_id = self.config.l2_chain_id;
        let mut multiaddr = libp2p::Multiaddr::from(gossip_addr.ip());
        multiaddr.push(libp2p::multiaddr::Protocol::Tcp(gossip_addr.port()));
        Network::builder()
            .with_discovery_address(disc_addr)
            .with_chain_id(chain_id)
            .with_gossip_address(multiaddr)
            .with_unsafe_block_signer(signer)
            .with_keypair(keypair)
            .build()
            .map(Some)
            .map_err(RollupNodeError::Network)
    }

    async fn init_derivation(&self) -> Result<(L2ForkchoiceState, OnlinePipeline), Self::Error> {
        // Create the caching L1/L2 EL providers for derivation.
        let mut l1_derivation_provider =
            AlloyChainProvider::new(self.l1_provider.clone(), DERIVATION_PROVIDER_CACHE_SIZE);
        let mut l2_derivation_provider = AlloyL2ChainProvider::new(
            self.l2_provider.clone(),
            self.config.clone(),
            DERIVATION_PROVIDER_CACHE_SIZE,
        );

        // Find the starting forkchoice state.
        let starting_forkchoice = find_starting_forkchoice(
            self.config.as_ref(),
            &mut l1_derivation_provider,
            &mut l2_derivation_provider,
        )
        .await?;

        info!(
            target: "rollup_node",
            unsafe = %starting_forkchoice.un_safe.block_info.number,
            safe = %starting_forkchoice.safe.block_info.number,
            finalized = %starting_forkchoice.finalized.block_info.number,
            "Found starting forkchoice state"
        );

        // Start the derivation pipeline's L1 origin 1 channel timeout before the L1 origin of the
        // safe head block.
        let starting_origin_num = starting_forkchoice.safe.l1_origin.number.saturating_sub(
            self.config.channel_timeout(starting_forkchoice.safe.block_info.timestamp),
        );
        let starting_origin =
            l1_derivation_provider.block_info_by_number(starting_origin_num).await?;

        let pipeline = OnlinePipeline::new(
            self.config.clone(),
            starting_forkchoice.safe,
            starting_origin,
            OnlineBlobProvider::init(self.l1_beacon.clone()).await,
            l1_derivation_provider,
            l2_derivation_provider,
        )
        .await?;

        Ok((starting_forkchoice, pipeline))
    }
}

#[async_trait]
impl SequencerNodeService for RollupNode {
    async fn start(&self) -> Result<(), Self::Error> {
        unimplemented!()
    }
}

/// Errors that can occur during the operation of the [RollupNode].
#[derive(Error, Debug)]
pub enum RollupNodeError {
    /// An error occurred while finding the sync starting point.
    #[error(transparent)]
    SyncStart(#[from] SyncStartError),
    /// An error occurred while creating the derivation pipeline.
    #[error(transparent)]
    OnlinePipeline(#[from] PipelineErrorKind),
    /// An error occurred while initializing the derivation pipeline.
    #[error(transparent)]
    AlloyChainProvider(#[from] AlloyChainProviderError),
    /// An error occured while initializing the network.
    #[error(transparent)]
    Network(#[from] NetworkBuilderError),
}
