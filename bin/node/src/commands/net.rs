//! Net Subcommand

use crate::flags::{GlobalArgs, P2PArgs, RpcArgs};
use clap::Parser;
use futures::future::OptionFuture;
use jsonrpsee::{RpcModule, server::Server};
use kona_p2p::{NetworkBuilder, P2pRpcRequest};
use kona_rpc::{NetworkRpc, OpP2PApiServer, RpcBuilder};
use tracing::{debug, info, warn};
use url::Url;

/// The `net` Subcommand
///
/// The `net` subcommand is used to run the networking stack for the `kona-node`.
///
/// # Usage
///
/// ```sh
/// kona-node net [FLAGS] [OPTIONS]
/// ```
#[derive(Parser, Default, PartialEq, Debug, Clone)]
#[command(about = "Runs the networking stack for the kona-node.")]
pub struct NetCommand {
    /// URL of the L1 execution client RPC API.
    /// This is used to load the unsafe block signer from runtime.
    /// Without this, the rollup config unsafe block signer will be used which may be outdated.
    #[arg(long, visible_alias = "l1", env = "L1_ETH_RPC")]
    pub l1_eth_rpc: Option<Url>,
    /// P2P CLI Flags
    #[command(flatten)]
    pub p2p: P2PArgs,
    /// RPC CLI Flags
    #[command(flatten)]
    pub rpc: RpcArgs,
}

impl NetCommand {
    /// Initializes the logging system based on global arguments.
    pub fn init_logs(&self, args: &GlobalArgs) -> anyhow::Result<()> {
        // Filter out discovery warnings since they're very very noisy.
        let filter = tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("discv5=error".parse()?)
            .add_directive("bootstore=debug".parse()?);

        // Initialize the telemetry stack.
        args.init_tracing(Some(filter))?;
        Ok(())
    }

    /// Run the Net subcommand.
    pub async fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        let signer = args.genesis_signer()?;
        info!(target: "net", "Genesis block signer: {:?}", signer);

        let rpc_config = RpcBuilder::from(self.rpc);

        let (handle, tx, rx) = if rpc_config.disabled {
            info!(target: "net", "RPC server disabled");
            (None, None, None)
        } else {
            info!(target: "net", socket = ?rpc_config.socket, "Starting RPC server");
            let (tx, rx) = tokio::sync::mpsc::channel(1024);

            // Setup the RPC server with the P2P RPC Module
            let mut launcher = RpcModule::new(());
            launcher.merge(NetworkRpc::new(tx.clone()).into_rpc())?;

            let server = Server::builder().build(rpc_config.socket).await?;
            (Some(server.start(launcher)), Some(tx), Some(rx))
        };

        // Get the rollup config from the args
        let rollup_config = args
            .rollup_config()
            .ok_or(anyhow::anyhow!("Rollup config not found for chain id: {}", args.l2_chain_id))?;

        // Start the Network Stack
        self.p2p.check_ports()?;
        let p2p_config = self.p2p.config(&rollup_config, args, self.l1_eth_rpc).await?;
        let mut network = NetworkBuilder::from(p2p_config).build()?;
        let mut recv = network.unsafe_block_recv();
        network.start(rx).await?;
        info!(target: "net", "Network started, receiving blocks.");

        // On an interval, use the rpc tx to request stats about the p2p network.
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));

        loop {
            tokio::select! {
                payload = recv.recv() => {
                    match payload {
                        Ok(payload) => info!(target: "net", "Received unsafe payload: {:?}", payload.payload.block_hash()),
                        Err(e) => debug!(target: "net", "Failed to receive unsafe payload: {:?}", e),
                    }
                }
                _ = interval.tick(), if tx.is_some() => {
                    let Some(ref sender) = tx else {
                        unreachable!("tx must be some (see above)");
                    };

                    let (otx, mut orx) = tokio::sync::oneshot::channel();
                    if let Err(e) = sender.send(P2pRpcRequest::PeerCount(otx)).await {
                        warn!(target: "net", "Failed to send network rpc request: {:?}", e);
                        continue;
                    }
                    tokio::time::timeout(tokio::time::Duration::from_secs(5), async move {
                        loop {
                            match orx.try_recv() {
                                Ok((d, g)) => {
                                    let d = d.unwrap_or_default();
                                    info!(target: "net", "Peer counts: Discovery={} | Swarm={}", d, g);
                                    break;
                                }
                                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                                    /* Keep trying to receive */
                                }
                                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                                    break;
                                }
                            }
                        }
                    }).await.unwrap();
                }
                _ = OptionFuture::from(handle.clone().map(|h| h.stopped())) => {
                    warn!(target: "net", "RPC server stopped");
                    return Ok(());
                }
            }
        }
    }
}
