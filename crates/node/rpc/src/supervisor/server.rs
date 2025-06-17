//! RPC module for the kona-node supervisor event stream.

use crate::SupervisorEventsServer;
use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use jsonrpsee::{
    core::SubscriptionError,
    server::{PendingSubscriptionSink, ServerHandle, SubscriptionMessage},
};
use std::net::SocketAddr;
use tokio::sync::broadcast;

/// The supervisor rpc for the kona-node.
#[derive(Debug)]
pub struct SupervisorRpcServer {
    /// A channel to receive managed events from the node.
    events: broadcast::Receiver<()>,
    /// A JWT token for authentication.
    #[allow(dead_code)]
    jwt_token: JwtSecret,
    /// The socket address for the RPC server.
    socket: SocketAddr,
}

impl SupervisorRpcServer {
    /// Creates a new instance of the `SupervisorRpcServer`.
    pub const fn new(
        events: broadcast::Receiver<()>,
        jwt_token: JwtSecret,
        socket: SocketAddr,
    ) -> Self {
        Self { events, jwt_token, socket }
    }

    /// Returns the socket address for the RPC server.
    pub const fn socket(&self) -> SocketAddr {
        self.socket
    }

    /// Launches the RPC server with the given socket address.
    pub async fn launch(self) -> std::io::Result<ServerHandle> {
        // let secret = self.jwt_token.clone();
        // let rpc_middleware =
        //     RpcServiceBuilder::new().layer_fn(move |service| AuthMiddleware::new(secret,
        // service));
        let server = jsonrpsee::server::ServerBuilder::default()
            // .set_rpc_middleware(rpc_middleware)
            .build(self.socket)
            .await?;

        Ok(server.start(self.into_rpc()))
    }
}

#[async_trait]
impl SupervisorEventsServer for SupervisorRpcServer {
    async fn ws_event_stream(
        &self,
        sink: PendingSubscriptionSink,
    ) -> Result<(), SubscriptionError> {
        let mut events = self.events.resubscribe();
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
