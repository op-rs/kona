//! Discovery Module.

use std::time::Duration;
use tokio::{
    sync::mpsc::{Receiver, channel},
    time::sleep,
};

use discv5::{Discv5, Enr};

use crate::{BootNodes, Discv5Builder, Discv5Wrapper, OpStackEnr};

/// The number of peers to buffer in the channel.
const DISCOVERY_PEER_CHANNEL_SIZE: usize = 256;

/// The discovery driver handles running the discovery service.
pub struct Discv5Driver {
    /// The [Discv5] discovery service.
    pub disc: Discv5Wrapper,
    /// The chain ID of the network.
    pub chain_id: u64,
    ///
    /// The interval to discovery random nodes.
    pub interval: Duration,
}

impl Discv5Driver {
    /// Returns a new [`Discv5Builder`] instance.
    pub fn builder() -> Discv5Builder {
        Discv5Builder::new()
    }

    /// Instantiates a new [`Discv5Driver`].
    pub fn new(disc: Discv5, chain_id: u64) -> Self {
        Self { disc: Discv5Wrapper::new(disc), chain_id, interval: Duration::from_secs(10) }
    }

    /// Spawns a new [`Discv5`] discovery service in a new tokio task.
    ///
    /// Returns a [`Receiver`] to receive [`Enr`] structs.
    ///
    /// ## Errors
    ///
    /// Returns an error if the address or chain ID is not set
    /// on the [`Discv5Builder`].
    ///
    /// ## Example
    ///
    /// ```no_run
    /// use kona_p2p::Discv5Driver;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
    ///     let mut disc = Discv5Driver::builder()
    ///         .with_address(socket)
    ///         .with_chain_id(10) // OP Mainnet chain id
    ///         .build()
    ///         .expect("Failed to build discovery service");
    ///     let mut peer_recv = disc.start();
    ///
    ///     loop {
    ///         if let Some(peer) = peer_recv.recv().await {
    ///             println!("Received peer: {:?}", peer);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn start(&self) -> Receiver<Enr> {
        // Clone the bootnodes since the spawned thread takes mutable ownership.
        let bootnodes = BootNodes::from_chain_id(self.chain_id);

        // Create a multi-producer, single-consumer (mpsc) channel to receive
        // peers bounded by `DISCOVERY_PEER_CHANNEL_SIZE`.
        let (sender, recv) = channel::<Enr>(DISCOVERY_PEER_CHANNEL_SIZE);

        let chain_id = self.chain_id;
        let interval = self.interval;
        let mut discv5 = self.disc.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = discv5.start().await {
                    warn!(target: "p2p::discv5::driver", "Failed to start discovery service: {:?}", e);
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
                break;
            }

            if let Err(e) = discv5.bootstrap(&bootnodes).await {
                warn!(target: "p2p::discv5::driver", "Failed to bootstrap discovery service: {:?}", e);
                return;
            }

            info!(target: "p2p::discv5::driver", "Started peer discovery");

            loop {
                match discv5.find_node().await {
                    Ok(nodes) => {
                        let enrs =
                            nodes.iter().filter(|node| OpStackEnr::is_valid_node(node, chain_id));

                        for enr in enrs {
                            _ = sender.send(enr.clone()).await;
                        }
                    }
                    Err(err) => {
                        warn!(target: "p2p::discv5::driver", "discovery error: {:?}", err);
                    }
                }

                sleep(interval).await;
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
    async fn test_discv5_driver() {
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let discovery = Discv5Driver::builder()
            .with_address(socket)
            .with_chain_id(10)
            .build()
            .expect("Failed to build discovery service");
        let _ = discovery.start();
        // The service starts.
        // TODO: verify with a heartbeat.
    }
}
