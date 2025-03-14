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
use kona_p2p::driver::NetworkDriver;
use kona_registry::ROLLUP_CONFIGS;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tracing_subscriber::EnvFilter;

/// The gossip command.
#[derive(Parser, Debug, Clone)]
#[command(about = "Runs the gossip service")]
pub struct GossipCommand {
    /// Verbosity level (0-2)
    #[arg(long, short, action = ArgAction::Count)]
    pub v: u8,
    /// The L2 chain ID to use.
    #[clap(long, short = 'c', default_value = "10", help = "The L2 chain ID to use")]
    pub l2_chain_id: u64,
    /// Port to listen for gossip on.
    #[clap(long, short = 'l', default_value = "9099", help = "Port to listen for gossip on")]
    pub gossip_port: u16,
    /// Interval to send discovery packets.
    #[clap(long, short = 'i', default_value = "1", help = "Interval to send discovery packets")]
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

        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), self.gossip_port);
        tracing::info!("Starting gossip driver on {:?}", socket);

        let mut driver = NetworkDriver::builder()
            .with_chain_id(self.l2_chain_id)
            .with_unsafe_block_signer(signer)
            .with_gossip_addr(socket)
            .with_interval(std::time::Duration::from_secs(self.interval))
            .build()?;
        let recv =
            driver.take_unsafe_block_recv().ok_or(anyhow::anyhow!("No unsafe block receiver"))?;
        driver.start()?;
        tracing::info!("Gossip driver started, receiving blocks.");
        loop {
            match recv.recv() {
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
