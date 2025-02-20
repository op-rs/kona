//! Discovery subcommand for Hilo.

use crate::globals::GlobalArgs;
use clap::Args;
use kona_net::discovery::builder::DiscoveryBuilder;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// The discovery subcommand.
#[derive(Debug, Clone, Args)]
#[non_exhaustive]
pub struct DiscCommand {
    /// Port to listen for gossip on.
    #[clap(long, short = 'l', default_value = "9099", help = "Port to listen for gossip on")]
    pub gossip_port: u16,
    /// Interval to send discovery packets.
    #[clap(long, short = 'i', default_value = "1", help = "Interval to send discovery packets")]
    pub interval: u64,
}

impl DiscCommand {
    /// Run the discovery subcommand.
    pub async fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), self.gossip_port);
        tracing::info!("Starting discovery service on {:?}", socket);

        let mut discovery_builder =
            DiscoveryBuilder::new().with_address(socket).with_chain_id(args.l2_chain_id);
        let mut discovery = discovery_builder.build()?;
        discovery.interval = std::time::Duration::from_secs(self.interval);
        let mut peer_recv = discovery.start();
        tracing::info!("Discovery service started, receiving peers.");

        loop {
            match peer_recv.recv().await {
                Some(peer) => {
                    tracing::info!("Received peer: {:?}", peer);
                }
                None => {
                    tracing::warn!("Failed to receive peer");
                }
            }
        }
    }
}
