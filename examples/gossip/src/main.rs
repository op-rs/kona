//! Example of how to use kona's libp2p gossip service as a standalone component
//!
//! ## Usage
//!
//! ```sh
//! cargo run --release -p example-gossip
//! ```
//!
//! ## Inputs
//!
//! The gossip service takes the following inputs:
//!
//! - `-v` or `--verbosity`: Verbosity level (0-2)
//! - `-c` or `--l2-chain-id`: The L2 chain ID to use
//! - `-l` or `--gossip-port`: Port to listen for gossip on
//! - `-i` or `--interval`: Interval to send discovery packets

#![warn(unused_crate_dependencies)]

use clap::{ArgAction, Parser};
use kona_cli::init_tracing_subscriber;
use kona_p2p::{AdvertisedIpAndPort, Network};
use kona_registry::ROLLUP_CONFIGS;
use libp2p::Multiaddr;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tracing_subscriber::EnvFilter;

/// The gossip command.
#[derive(Parser, Debug, Clone)]
#[command(about = "Runs the gossip service")]
pub struct GossipCommand {
    /// Verbosity level (0-5).
    /// If set to 0, no logs are printed.
    /// By default, the verbosity level is set to 3 (info level).
    #[arg(long, short, default_value = "3", action = ArgAction::Count)]
    pub v: u8,
    /// The L2 chain ID to use.
    #[arg(long, short = 'c', default_value = "10", help = "The L2 chain ID to use")]
    pub l2_chain_id: u64,
    /// Port to listen for gossip on.
    #[arg(long, short = 'l', default_value = "9099", help = "Port to listen for gossip on")]
    pub gossip_port: u16,
    /// Port to listen for discovery on.
    #[arg(long, short = 'd', default_value = "9098", help = "Port to listen for discovery on")]
    pub disc_port: u16,
    /// Interval to send discovery packets.
    #[arg(long, short = 'i', default_value = "1", help = "Interval to send discovery packets")]
    pub interval: u64,
}

impl GossipCommand {
    /// Run the gossip subcommand.
    pub async fn run(self) -> anyhow::Result<()> {
        init_tracing_subscriber(self.v, None::<EnvFilter>)?;

        let signer = ROLLUP_CONFIGS
            .get(&self.l2_chain_id)
            .ok_or(anyhow::anyhow!("No rollup config found for chain ID"))?
            .genesis
            .system_config
            .as_ref()
            .ok_or(anyhow::anyhow!("No system config found for chain ID"))?
            .batcher_address;
        tracing::info!("Gossip configured with signer: {:?}", signer);

        let gossip = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), self.gossip_port);
        tracing::info!("Starting gossip driver on {:?}", gossip);

        let mut gossip_addr = Multiaddr::from(gossip.ip());
        gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(gossip.port()));
        let disc_addr = AdvertisedIpAndPort::new(
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            self.disc_port,
            self.disc_port,
        );
        let mut network = Network::builder()
            .with_discovery_address(disc_addr)
            .with_chain_id(self.l2_chain_id)
            .with_gossip_address(gossip_addr)
            .with_unsafe_block_signer(signer)
            .build()?;

        let mut recv = network.unsafe_block_recv();
        network.start()?;
        tracing::info!("Gossip driver started, receiving blocks.");
        loop {
            match recv.recv().await {
                Ok(block) => {
                    tracing::info!("Received unsafe block: {:?}", block);
                }
                Err(e) => {
                    tracing::warn!("Failed to receive unsafe block: {:?}", e);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(err) = GossipCommand::parse().run().await {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
    Ok(())
}
