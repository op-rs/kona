//! Discovery Module.

use std::time::Duration;
use tokio::{
    sync::mpsc::{channel, Receiver},
    time::sleep,
};
use tracing::{info, warn};

use discv5::{enr::NodeId, Discv5};

use crate::{
    discovery::{bootnodes::BOOTNODES, builder::DiscoveryBuilder},
    types::{enr::OpStackEnr, peer::Peer},
};

/// The number of peers to buffer in the channel.
const DISCOVERY_PEER_CHANNEL_SIZE: usize = 256;

/// The discovery driver handles running the discovery service.
pub struct DiscoveryDriver {
    /// The [Discv5] discovery service.
    pub disc: Discv5,
    /// The chain ID of the network.
    pub chain_id: u64,
    /// The interval to discovery random nodes.
    pub interval: Duration,
}

impl DiscoveryDriver {
    /// Returns a new [DiscoveryBuilder] instance.
    pub fn builder() -> DiscoveryBuilder {
        DiscoveryBuilder::new()
    }

    /// Instantiates a new [DiscoveryDriver].
    pub fn new(disc: Discv5, chain_id: u64) -> Self {
        Self { disc, chain_id, interval: Duration::from_secs(10) }
    }

    /// Spawns a new [Discv5] discovery service in a new tokio task.
    ///
    /// Returns a [Receiver] to receive [Peer] structs.
    ///
    /// ## Errors
    ///
    /// Returns an error if the address or chain ID is not set
    /// on the [crate::discovery::builder::DiscoveryBuilder].
    ///
    /// ## Example
    ///
    /// ```no_run
    /// use kona_net::discovery::builder::DiscoveryBuilder;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
    ///     let mut discovery = DiscoveryBuilder::new()
    ///         .with_address(socket)
    ///         .with_chain_id(10) // OP Mainnet chain id
    ///         .build()
    ///         .expect("Failed to build discovery service");
    ///     let mut peer_recv = discovery.start();
    ///
    ///     loop {
    ///         if let Some(peer) = peer_recv.recv().await {
    ///             println!("Received peer: {:?}", peer);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn start(mut self) -> Receiver<Peer> {
        // Clone the bootnodes since the spawned thread takes mutable ownership.
        let bootnodes = BOOTNODES.clone();

        // Create a multi-producer, single-consumer (mpsc) channel to receive
        // peers bounded by `DISCOVERY_PEER_CHANNEL_SIZE`.
        let (sender, recv) = channel::<Peer>(DISCOVERY_PEER_CHANNEL_SIZE);

        tokio::spawn(async move {
            bootnodes.into_iter().for_each(|enr| _ = self.disc.add_enr(enr));
            loop {
                if let Err(e) = self.disc.start().await {
                    warn!("Failed to start discovery service: {:?}", e);
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
                break;
            }

            info!("Started peer discovery");

            loop {
                let target = NodeId::random();
                match self.disc.find_node(target).await {
                    Ok(nodes) => {
                        let peers = nodes
                            .iter()
                            .filter(|node| OpStackEnr::is_valid_node(node, self.chain_id))
                            .flat_map(Peer::try_from);

                        for peer in peers {
                            _ = sender.send(peer).await;
                        }
                    }
                    Err(err) => {
                        warn!("discovery error: {:?}", err);
                    }
                }

                sleep(self.interval).await;
            }
        });

        recv
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[tokio::test]
    async fn test_discovery_driver() {
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let discovery = DiscoveryBuilder::new()
            .with_address(socket)
            .with_chain_id(10)
            .build()
            .expect("Failed to build discovery service");
        let _ = discovery.start();
        // The service starts.
        // TODO: verify with a heartbeat.
    }
}
