use std::{io, net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use jsonrpsee::server::ServerBuilder;
use kona_supervisor_core::{SupervisorRpc, SupervisorService};
use kona_supervisor_rpc::SupervisorApiServer;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::SupervisorActor;

pub struct SupervisorRpcActor<T> {
    rpc_addr: SocketAddr,
    supervisor: Arc<T>,
    cancel_token: CancellationToken,
}

impl<T> SupervisorRpcActor<T>
where
    T: SupervisorService + 'static,
{
    pub fn new(rpc_addr: SocketAddr, supervisor: Arc<T>, cancel_token: CancellationToken) -> Self {
        Self { rpc_addr, supervisor, cancel_token }
    }
}

#[async_trait]
impl<T> SupervisorActor for SupervisorRpcActor<T>
where
    T: SupervisorService + 'static,
{
    type InboundEvent = ();
    type Error = io::Error;

    async fn start(mut self) -> Result<(), Self::Error> {
        info!(
          target: "supervisor::rpc_actor",
          addr = %self.rpc_addr,
          "Starting Supervisor RPC Actor",
        );

        let supervisor_rpc = SupervisorRpc::new(self.supervisor.clone());
        let server = ServerBuilder::default().build(self.rpc_addr).await?;
        let handle = server.start(supervisor_rpc.into_rpc());

        tokio::select! {
            _ = self.cancel_token.cancelled() => {
                info!(target: "supervisor::rpc_actor", "Cancellation requested, stopping RPC server...");
            }
            _ = handle.stopped() => {
                info!(target: "supervisor::rpcactor", "Supervisor RPC server stopped gracefully");
            }
        }

        Ok(())
    }
}
