//! Bootstore Subcommand

use crate::flags::GlobalArgs;
use clap::Parser;
use kona_p2p::BootStore;
use std::path::PathBuf;

/// The `bootstore` Subcommand
///
/// The `bootstore` subcommand can be used to interact with local bootstores.
///
/// # Usage
///
/// ```sh
/// kona-node bootstore [FLAGS] [OPTIONS]
/// ```
#[derive(Parser, Default, PartialEq, Debug, Clone)]
#[command(about = "Utility tool to interact with local bootstores")]
pub struct BootstoreCommand {
    /// Optionally prints all bootstores.
    /// This option overrides the chain ID configured with `--l2-chain-id`.
    #[arg(long = "all")]
    pub all: bool,
    /// The directory to store the bootstore.
    #[arg(long = "p2p.bootstore", env = "KONA_NODE_P2P_BOOTSTORE")]
    pub bootstore: Option<PathBuf>,
}

impl BootstoreCommand {
    /// Initializes the logging system based on global arguments.
    pub fn init_logs(&self, args: &GlobalArgs) -> anyhow::Result<()> {
        args.init_tracing(None)?;
        Ok(())
    }

    /// Runs the subcommand.
    pub fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        println!("--------------------------");
        if self.all {
            self.all()?;
        } else {
            self.info(args.l2_chain_id)?;
        }
        Ok(())
    }

    /// Prints all bootstores.
    pub fn all(&self) -> anyhow::Result<()> {
        for available in BootStore::available(self.bootstore.clone()) {
            self.info(available)?;
        }
        Ok(())
    }

    /// Prints information for the bootstore with the given chain ID.
    pub fn info(&self, chain_id: u64) -> anyhow::Result<()> {
        let chain = kona_registry::OPCHAINS
            .get(&chain_id)
            .ok_or(anyhow::anyhow!("Chain ID {} not found in the registry", chain_id))?;
        println!("{} Bootstore (Chain ID: {})", chain.name, chain_id);
        let bootstore = BootStore::from_chain_id(chain_id, self.bootstore.clone(), vec![]);
        println!("Path: {}", bootstore.path.display());
        println!("Peer Count: {}", bootstore.peers.len());
        println!("Valid peers: {}", bootstore.valid_peers_with_chain_id(chain_id).len());
        println!("--------------------------");
        Ok(())
    }
}
