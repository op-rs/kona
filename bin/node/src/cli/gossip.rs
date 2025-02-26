//! Gossip subcommand.

use crate::cli::globals::GlobalArgs;
use clap::Args;
use kona_net::driver::NetworkDriver;
use kona_registry::ROLLUP_CONFIGS;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// The gossip subcommand.
#[derive(Debug, Clone, Args)]
#[non_exhaustive]
pub struct GossipCommand {
    /// Port to listen for gossip on.
    #[clap(long, short = 'l', default_value = "9099", help = "Port to listen for gossip on")]
    pub gossip_port: u16,
    /// Interval to send discovery packets.
    #[clap(long, short = 'i', default_value = "1", help = "Interval to send discovery packets")]
    pub interval: u64,
}

impl GossipCommand {
    /// Run the gossip subcommand.
    pub async fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        let signer = ROLLUP_CONFIGS
            .get(&args.l2_chain_id)
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
            .with_chain_id(args.l2_chain_id)
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
