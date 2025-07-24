//! RPC module for the kona-node supervisor event stream.

#[cfg(feature = "server")]
use crate::jsonrpsee::{SupervisorEvents, SubscriptionResult};
#[cfg(feature = "server")]
use alloy_rpc_types_engine::JwtSecret;
#[cfg(feature = "server")]
use async_trait::async_trait;
#[cfg(feature = "server")]
use jsonrpsee::server::ServerHandle;
#[cfg(feature = "server")]
use kona_interop::{ControlEvent, ManagedEvent};
#[cfg(feature = "server")]
use std::net::SocketAddr;
#[cfg(feature = "server")]
use tokio::sync::broadcast;

/// The supervisor rpc for the kona-node.
#[cfg(feature = "server")]
#[derive(Debug)]
pub struct SupervisorRpcServer {
    /// A channel to receive [`ManagedEvent`] from the node.
    managed_events: broadcast::Receiver<ManagedEvent>,

    // TODO: use this sender for http rpc queries
    /// A channel to send [`ControlEvent`].
    #[allow(dead_code)]
    control_events: broadcast::Sender<ControlEvent>,
    /// A JWT token for authentication.
    #[allow(dead_code)]
    jwt_token: JwtSecret,
    /// The socket address for the RPC server.
    socket: SocketAddr,
}

#[cfg(feature = "server")]
impl SupervisorRpcServer {
    /// Creates a new instance of the `SupervisorRpcServer`.
    pub const fn new(
        managed_events: broadcast::Receiver<ManagedEvent>,
        control_events: broadcast::Sender<ControlEvent>,
        jwt_token: JwtSecret,
        socket: SocketAddr,
    ) -> Self {
        Self { managed_events, control_events, jwt_token, socket }
    }

    /// Returns the socket address for the RPC server.
    pub const fn socket(&self) -> SocketAddr {
        self.socket
    }

    /// Launches the RPC server with the given socket address.
    pub async fn launch(self) -> std::io::Result<ServerHandle> {
        let server = jsonrpsee::server::ServerBuilder::default().build(self.socket).await?;

        Ok(server.start(self.into_rpc()))
    }
}

#[cfg(feature = "server")]
#[async_trait]
impl SupervisorEvents for SupervisorRpcServer {
    async fn ws_event_stream(&self) -> SubscriptionResult {
        // This is a placeholder implementation for subscription
        // The actual implementation would be handled by the jsonrpsee proc macro
        Ok(())
    }
}