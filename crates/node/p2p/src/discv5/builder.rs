//! Contains a builder for the discovery service.

use discv5::{Config, Discv5, Enr, enr::CombinedKey};
use std::{net::IpAddr, path::PathBuf};
use tokio::time::Duration;

use crate::{Discv5BuilderError, Discv5Driver, OpStackEnr};

/// The advertised address and ports for the discovery service.
#[derive(Debug, Clone)]
pub struct AdvertisedIpAndPort {
    /// The IP address to advertise.
    pub ip: IpAddr,
    /// The TCP port to advertise.
    pub tcp_port: u16,
    /// Fallback UDP port.
    pub udp_port: u16,
}

impl AdvertisedIpAndPort {
    /// Creates a new [`AdvertisedIpAndPort`] instance.
    pub const fn new(ip: IpAddr, tcp_port: u16, udp_port: u16) -> Self {
        Self { ip, tcp_port, udp_port }
    }
}

impl AdvertisedIpAndPort {
    /// Build the local node ENR. This should contain the information we wish to
    /// broadcast to the other nodes in the network. See
    /// [the op-node implementation](https://github.com/ethereum-optimism/optimism/blob/174e55f0a1e73b49b80a561fd3fedd4fea5770c6/op-node/p2p/discovery.go#L61-L97)
    /// for the go equivalent
    fn build_enr(&self, key: &CombinedKey, chain_id: u64) -> Result<Enr, discv5::enr::Error> {
        let opstack = OpStackEnr::from_chain_id(chain_id);
        let mut opstack_data = Vec::new();
        use alloy_rlp::Encodable;
        opstack.encode(&mut opstack_data);

        let mut enr_builder = Enr::builder();
        enr_builder.add_value_rlp(OpStackEnr::OP_CL_KEY, opstack_data.into());
        match self.ip {
            IpAddr::V4(addr) => {
                enr_builder.ip4(addr).tcp4(self.tcp_port).udp4(self.udp_port);
            }
            IpAddr::V6(addr) => {
                enr_builder.ip6(addr).tcp6(self.tcp_port).udp6(self.udp_port);
            }
        }

        enr_builder.build(key)
    }
}

/// Discovery service builder.
#[derive(Debug, Default, Clone)]
pub struct Discv5Builder {
    /// The discovery service advertised address.
    advertised_address: Option<AdvertisedIpAndPort>,
    /// The chain ID of the network.
    chain_id: Option<u64>,
    /// The interval to find peers.
    interval: Option<Duration>,
    /// The discovery config for the discovery service.
    discovery_config: Option<Config>,
    /// An optional path to the bootstore.
    bootstore: Option<PathBuf>,
    /// Additional bootnodes to manually add to the initial bootstore
    bootnodes: Vec<Enr>,
}

impl Discv5Builder {
    /// Creates a new [`Discv5Builder`] instance.
    pub const fn new() -> Self {
        Self {
            advertised_address: None,
            chain_id: None,
            interval: None,
            discovery_config: None,
            bootstore: None,
            bootnodes: Vec::new(),
        }
    }

    /// Sets the bootstore path.
    pub fn with_bootstore(mut self, bootstore: PathBuf) -> Self {
        self.bootstore = Some(bootstore);
        self
    }

    /// Sets the initial bootnodes to add to the bootstore.
    pub fn with_bootnodes(mut self, bootnodes: Vec<Enr>) -> Self {
        self.bootnodes = bootnodes;
        self
    }

    /// Sets the discovery service advertised address and ports.
    pub const fn with_address(mut self, address: AdvertisedIpAndPort) -> Self {
        self.advertised_address = Some(address);
        self
    }

    /// Sets the chain ID of the network.
    pub const fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Sets the interval to find peers.
    pub const fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    /// Sets the discovery config for the discovery service.
    pub fn with_discovery_config(mut self, config: Config) -> Self {
        self.discovery_config = Some(config);
        self
    }

    /// Builds a [`Discv5Driver`].
    pub fn build(self) -> Result<Discv5Driver, Discv5BuilderError> {
        let chain_id = self.chain_id.ok_or(Discv5BuilderError::ChainIdNotSet)?;

        let config = self.discovery_config.ok_or(Discv5BuilderError::DiscoveryConfigNotSet)?;

        let key = CombinedKey::generate_secp256k1();

        let enr = self
            .advertised_address
            .ok_or(Discv5BuilderError::AdvertisedAddrNotSet)?
            .build_enr(&key, chain_id)
            .map_err(|_| Discv5BuilderError::EnrBuildFailed)?;

        let interval = self.interval.unwrap_or(Duration::from_secs(5));
        let disc =
            Discv5::new(enr, key, config).map_err(|_| Discv5BuilderError::Discv5CreationFailed)?;

        Ok(Discv5Driver::new(disc, interval, chain_id, self.bootstore.clone(), self.bootnodes))
    }
}

#[cfg(test)]
mod tests {
    use discv5::{ConfigBuilder, ListenConfig};

    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_builds_valid_enr() {
        let addr = AdvertisedIpAndPort::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099, 9099);
        let mut builder = Discv5Builder::new();
        builder = builder.with_address(addr);
        builder = builder.with_chain_id(10);
        builder = builder.with_discovery_config(
            ConfigBuilder::new(ListenConfig::Ipv4 { ip: Ipv4Addr::UNSPECIFIED, port: 9099 })
                .build(),
        );
        let driver = builder.build().unwrap();
        let enr = driver.disc.local_enr();
        assert!(OpStackEnr::is_valid_node(&enr, 10));
    }
}
