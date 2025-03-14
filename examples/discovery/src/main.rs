//! Example of how to use kona's discv5 discovery service as a standalone component
//!
//! ## Usage
//!
//! ```sh
//! cargo run --release -p example-discovery
//! ```
//!
//! ## Inputs
//!
//! The discovery service takes the following inputs:
//!
//! - `-v` or `--verbosity`: Verbosity level (0-2)
//! - `-c` or `--l2-chain-id`: The L2 chain ID to use
//! - `-l` or `--disc-port`: Port to listen for discovery on
//! - `-i` or `--interval`: Interval to send discovery packets

#![warn(unused_crate_dependencies)]

use clap::{ArgAction, Parser};
use kona_cli::init_tracing_subscriber;
use kona_p2p::Discv5Builder;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tracing_subscriber::EnvFilter;

/// The discovery command.
#[derive(Parser, Debug, Clone)]
#[command(about = "Runs the discovery service")]
pub struct DiscCommand {
    /// Verbosity level (0-2)
    #[arg(long, short, action = ArgAction::Count)]
    pub v: u8,
    /// The L2 chain ID to use.
    #[clap(long, short = 'c', default_value = "10", help = "The L2 chain ID to use")]
    pub l2_chain_id: u64,
    /// Discovery port to listen on.
    #[clap(long, short = 'l', default_value = "9099", help = "Port to listen to discovery")]
    pub disc_port: u16,
    /// Interval to send discovery packets.
    #[clap(long, short = 'i', default_value = "1", help = "Interval to send discovery packets")]
    pub interval: u64,
}

impl DiscCommand {
    /// Run the discovery subcommand.
    pub async fn run(self) -> anyhow::Result<()> {
        use tracing_subscriber::filter::LevelFilter;
        let filter =
            EnvFilter::builder().with_default_directive(LevelFilter::INFO.into()).parse("")?;
        init_tracing_subscriber(self.v, Some(filter))?;

        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), self.disc_port);
        tracing::info!("Starting discovery service on {:?}", socket);

        let mut discovery_builder =
            Discv5Builder::new().with_address(socket).with_chain_id(self.l2_chain_id);
        let mut discovery = discovery_builder.build()?;
        discovery.interval = std::time::Duration::from_secs(self.interval);
        let mut handler = discovery.start();
        tracing::info!("Discovery service started, receiving peers.");

        // Every 10 seconds, print the peer stats from the discovery service.
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));

        loop {
            tokio::select! {
                enr = handler.enr_receiver.recv() => {
                    match enr {
                        Some(enr) => {
                            tracing::info!("Received peer: {:?}", enr);
                        }
                        None => {
                            tracing::warn!("Failed to receive peer");
                        }
                    }
                }
                _ = interval.tick() => {
                    let metrics = handler.metrics().await;
                    tracing::info!("Discovery metrics: {:?}", metrics);
                    let peers = handler.peers().await;
                    tracing::info!("Discovery peer count: {:?}", peers);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(err) = DiscCommand::parse().run().await {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
    Ok(())
}
