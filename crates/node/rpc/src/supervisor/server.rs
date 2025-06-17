//! RPC module for the kona-node supervisor event stream.

use crate::SupervisorEventsServer;
use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use jsonrpsee::{
    core::SubscriptionError,
    server::{PendingSubscriptionSink, ServerHandle, SubscriptionMessage},
};
use kona_interop::{ControlEvent, ManagedEvent};
use std::net::SocketAddr;
use tokio::sync::broadcast;

/// The supervisor rpc for the kona-node.
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

#[async_trait]
impl SupervisorEventsServer for SupervisorRpcServer {
    async fn ws_event_stream(
        &self,
        sink: PendingSubscriptionSink,
    ) -> Result<(), SubscriptionError> {
        let mut events = self.managed_events.resubscribe();
        tokio::spawn(async move {
            let sub = match sink.accept().await {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("Failed to accept subscription: {:?}", err);
                    return;
                }
            };
            let id = sub.subscription_id();
            loop {
                match events.recv().await {
                    Ok(_) => {
                        let Ok(message) = SubscriptionMessage::new(
                            "event",
                            id.clone(),
                            &String::from("Event received"),
                        ) else {
                            eprintln!("Failed to create subscription message");
                            break;
                        };
                        if let Err(err) = sub.send(message).await {
                            eprintln!("Client disconnected or error: {:?}", err);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        eprintln!("Lagged events, skipping...");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        eprintln!("Channel closed, stopping subscription.");
                        break;
                    }
                }
            }
        });
        Ok(())
    }
}
