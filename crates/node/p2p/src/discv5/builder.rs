//! Contains a builder for the discovery service.

use discv5::{
    Config, ConfigBuilder, Discv5, ListenConfig,
    enr::{CombinedKey, Enr},
};
use std::net::SocketAddr;

use crate::{Discv5Driver, OpStackEnr};

/// An error that can occur when building the discovery service.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Discv5BuilderError {
    /// The chain ID is not set.
    #[error("chain ID not set")]
    ChainIdNotSet,
    /// The listen config is not set.
    #[error("listen config not set")]
    ListenConfigNotSet,
    /// Could not create the discovery service.
    #[error("could not create discovery service")]
    Discv5CreationFailed,
    /// Failed to build the ENR.
    #[error("failed to build ENR")]
    EnrBuildFailed,
}

/// Discovery service builder.
#[derive(Debug, Default, Clone)]
pub struct Discv5Builder {
    /// The discovery service address.
    address: Option<SocketAddr>,
    /// The chain ID of the network.
    chain_id: Option<u64>,
    /// The listen config for the discovery service.
    listen_config: Option<ListenConfig>,

    /// The discovery config for the discovery service.
    discovery_config: Option<Config>,
}

impl Discv5Builder {
    /// Creates a new discovery builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the discovery service address.
    pub fn with_address(mut self, address: SocketAddr) -> Self {
        self.address = Some(address);
        self
    }

    /// Sets the chain ID of the network.
    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Sets the listen config for the discovery service.
    pub fn with_listen_config(mut self, listen_config: ListenConfig) -> Self {
        self.listen_config = Some(listen_config);
        self
    }

    /// Sets the discovery config for the discovery service.
    pub fn with_discovery_config(mut self, config: Config) -> Self {
        self.discovery_config = Some(config);
        self
    }

    /// Builds a [`Discv5Driver`].
    pub fn build(&mut self) -> Result<Discv5Driver, Discv5BuilderError> {
        let chain_id = self.chain_id.ok_or(Discv5BuilderError::ChainIdNotSet)?;
        let opstack = OpStackEnr::from_chain_id(chain_id);
        let mut opstack_data = Vec::new();
        use alloy_rlp::Encodable;
        opstack.encode(&mut opstack_data);

        let config = if let Some(mut discovery_config) = self.discovery_config.take() {
            if let Some(listen_config) = self.listen_config.take() {
                discovery_config.listen_config = listen_config;
            }
            Ok::<Config, Discv5BuilderError>(discovery_config)
        } else {
            let listen_config = self
                .listen_config
                .take()
                .or_else(|| self.address.map(ListenConfig::from))
                .ok_or(Discv5BuilderError::ListenConfigNotSet)?;
            Ok(ConfigBuilder::new(listen_config).build())
        }?;

        let key = CombinedKey::generate_secp256k1();
        let mut enr_builder = Enr::builder();
        enr_builder.add_value_rlp(OpStackEnr::OP_CL_KEY, opstack_data.into());
        match config.listen_config {
            ListenConfig::Ipv4 { ip, port } => {
                enr_builder.ip4(ip).tcp4(port);
            }
            ListenConfig::Ipv6 { ip, port } => {
                enr_builder.ip6(ip).tcp6(port);
            }
            ListenConfig::DualStack { ipv4, ipv4_port, ipv6, ipv6_port } => {
                enr_builder.ip4(ipv4).tcp4(ipv4_port);
                enr_builder.ip6(ipv6).tcp6(ipv6_port);
            }
        }
        let enr = enr_builder.build(&key).map_err(|_| Discv5BuilderError::EnrBuildFailed)?;

        let disc =
            Discv5::new(enr, key, config).map_err(|_| Discv5BuilderError::Discv5CreationFailed)?;

        Ok(Discv5Driver::new(disc, chain_id))
    }
}
