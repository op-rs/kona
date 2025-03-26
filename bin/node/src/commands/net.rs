//! Net Subcommand

use clap::Parser;
use kona_p2p::Network;
use std::net::SocketAddr;
use kona_rpc::OpP2PApiServer;
use kona_registry::ROLLUP_CONFIGS;
use libp2p::multiaddr::{Multiaddr, Protocol};
use crate::flags::{P2PArgs, RPCArgs, GlobalArgs};

/// The `net` Subcommand
///
/// The `net` subcommand is used to run the networking stack for the `kona-node`.
///
/// # Usage
///
/// ```sh
/// kona-node net [FLAGS] [OPTIONS]
/// ```
#[derive(Parser, Debug, Clone)]
#[command(about = "Runs the networking stack for the kona-node.")]
pub struct NetCommand {
    /// The L2 chain ID to use.
    #[arg(long, short = 'c', default_value = "10", help = "The L2 chain ID to use")]
    pub l2_chain_id: u64,
    /// P2P CLI Flags
    #[command(flatten)]
    pub p2p: P2PArgs,
    /// RPC CLI Flags
    #[command(flatten)]
    pub rpc: RPCArgs,
}

impl NetCommand {
    /// Run the Net subcommand.
    pub async fn run(self, _args: &GlobalArgs) -> anyhow::Result<()> {
        let signer = ROLLUP_CONFIGS
            .get(&self.l2_chain_id)
            .ok_or(anyhow::anyhow!("No rollup config found for chain ID"))?
            .genesis
            .system_config
            .as_ref()
            .ok_or(anyhow::anyhow!("No system config found for chain ID"))?
            .batcher_address;
        tracing::info!("Gossip configured with signer: {:?}", signer);

        let rpc_config: kona_rpc::RpcConfig = self.rpc.into();
        let socket: SocketAddr = (&rpc_config).into();
        let server = jsonrpsee::server::Server::builder()
            .build(socket)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to build jsonrpsee server: {:?}", e))?;
        let mut module = jsonrpsee::RpcModule::new(());

        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let p2p = kona_p2p::NetworkRpc::new(tx);
        module.merge(p2p.into_rpc()).map_err(|e| anyhow::anyhow!("Failed to merge RPC Modules: {:?}", e))?;
        let handle = server.start(module);
        tracing::info!("Started RPC server on {:?}", socket);

        let ip = self.p2p.listen_ip;
        let disc_addr = SocketAddr::new(ip, self.p2p.listen_udp_port);
        let mut gossip_addr = Multiaddr::from(ip);
        gossip_addr.push(Protocol::Tcp(self.p2p.listen_tcp_port));
        tracing::info!("Starting gossip driver on {:?}", gossip_addr);

        let mut network = Network::builder()
            .with_discovery_address(disc_addr)
            .with_chain_id(self.l2_chain_id)
            .with_gossip_address(gossip_addr)
            .with_unsafe_block_signer(signer)
            .with_rpc_receiver(rx)
            .build()?;

        let mut recv =
            network.take_unsafe_block_recv().ok_or(anyhow::anyhow!("No unsafe block receiver"))?;
        network.start()?;
        tracing::info!("Gossip driver started, receiving blocks.");
        loop {
            tokio::select! {
                _ = handle.clone().stopped() => {
                    tracing::warn!("RPC server stopped");
                    return Ok(());
                }
                block = recv.recv() => {
                    match block {
                        Some(block) => {
                            tracing::info!("Received unsafe block: {:?}", block);
                        }
                        None => {
                            tracing::warn!("Failed to receive unsafe block");
                        }
                    }
                }
            }
        }
    }
}
