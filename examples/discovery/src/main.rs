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
use discv5::enr::CombinedKey;
use kona_cli::init_tracing_subscriber;
use kona_p2p::{Discv5Builder, LocalNode};
use std::net::{IpAddr, Ipv4Addr};

/// The discovery command.
#[derive(Parser, Debug, Clone)]
#[command(about = "Runs the discovery service")]
pub struct DiscCommand {
    /// Verbosity level (0-5).
    /// If set to 0, no logs are printed.
    /// By default, the verbosity level is set to 3 (info level).
    #[arg(long, short, default_value = "3", action = ArgAction::Count)]
    pub v: u8,
    /// The L2 chain ID to use.
    #[arg(long, short = 'c', default_value = "10", help = "The L2 chain ID to use")]
    pub l2_chain_id: u64,
    /// Discovery port to listen on.
    #[arg(long, short = 'l', default_value = "9099", help = "Port to listen to discovery")]
    pub disc_port: u16,
    /// Interval to send discovery packets.
    #[arg(long, short = 'i', default_value = "3", help = "Interval to send discovery packets")]
    pub interval: u64,
}

impl DiscCommand {
    /// Run the discovery subcommand.
    pub async fn run(self) -> anyhow::Result<()> {
        let filter = tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("discv5=error".parse()?);
        init_tracing_subscriber(self.v, Some(filter))?;

        let CombinedKey::Secp256k1(secret_key) = CombinedKey::generate_secp256k1() else {
            unreachable!()
        };

        let socket = LocalNode::new(
            secret_key,
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            self.disc_port,
            self.disc_port,
        );
        tracing::info!("Starting discovery service on {:?}", socket);

        let discovery_builder =
            Discv5Builder::new().with_local_node(socket).with_chain_id(self.l2_chain_id);
        let mut discovery = discovery_builder.build()?;
        discovery.interval = std::time::Duration::from_secs(self.interval);
        discovery.forward = false;
        let (handler, mut enr_receiver) = discovery.start();
        tracing::info!("Discovery service started, receiving peers.");

        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            tokio::select! {
                enr = enr_receiver.recv() => {
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
                    let metrics = handler.metrics();
                    let peer_count = handler.peer_count();
                    tokio::spawn(async move {
                        if let Ok(metrics) = metrics.await {
                            tracing::info!("Discovery metrics: {:?}", metrics);
                        }
                        if let Ok(pc) = peer_count.await {
                            tracing::info!("Discovery peer count: {:?}", pc);
                        }
                    });
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
