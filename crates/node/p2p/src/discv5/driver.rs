//! Discovery Module.

use tokio::{
    sync::mpsc::channel,
    time::{Duration, sleep},
};

use discv5::{Discv5, Enr, Event, enr::NodeId};

use crate::{
    BootNode, BootNodes, Discv5Builder, Discv5Handler, HandlerRequest, HandlerResponse, OpStackEnr,
};

/// The [`Discv5Driver`] drives the discovery service.
///
/// Calling [`Discv5Driver::start`] spawns a new [`Discv5`]
/// discovery service in a new tokio task and returns a
/// [`Discv5Handler`].
///
/// Channels are used to communicate between the [`Discv5Handler`]
/// and the spawned task containing the [`Discv5`] service.
///
/// Since some requested operations are asynchronous, this pattern of message
/// passing is used as opposed to wrapping the [`Discv5`] in an `Arc<Mutex<>>`.
/// If an `Arc<Mutex<>>` were used, a lock held across the operation's future
/// would be needed since some asynchronous operations require a mutable
/// reference to the [`Discv5`] service.
///
/// ## Example
///
/// ```no_run
/// use kona_p2p::Discv5Driver;
/// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
///
/// #[tokio::main]
/// async fn main() {
///     let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
///     let mut disc = Discv5Driver::builder()
///         .with_address(socket)
///         .with_chain_id(10) // OP Mainnet chain id
///         .build()
///         .expect("Failed to build discovery service");
///     let mut handler = disc.start();
///
///     loop {
///         if let Some(enr) = handler.enr_receiver.recv().await {
///             println!("Received peer enr: {:?}", enr);
///         }
///     }
/// }
/// ```
pub struct Discv5Driver {
    /// The [`Discv5`] discovery service.
    pub disc: Discv5,
    /// The chain ID of the network.
    pub chain_id: u64,
    ///
    /// The interval to discovery random nodes.
    pub interval: Duration,
}

impl Discv5Driver {
    /// Returns a new [`Discv5Builder`] instance.
    pub fn builder() -> Discv5Builder {
        Discv5Builder::new()
    }

    /// Instantiates a new [`Discv5Driver`].
    pub fn new(disc: Discv5, chain_id: u64) -> Self {
        Self { disc, chain_id, interval: Duration::from_secs(10) }
    }

    /// Starts the inner [`Discv5`] service.
    async fn init(&mut self) {
        loop {
            if let Err(e) = self.disc.start().await {
                warn!(target: "p2p::discv5::driver", "Failed to start discovery service: {:?}", e);
                sleep(Duration::from_secs(2)).await;
                continue;
            }
            break;
        }
    }

    /// Bootstraps the [`Discv5`] service with the bootnodes.
    async fn bootstrap(&self) {
        let nodes = BootNodes::from_chain_id(self.chain_id);

        info!(target: "p2p::discv5", "Adding {} bootstrap nodes...", nodes.0.len());
        debug!(target: "p2p::discv5", ?nodes);

        for node in nodes.0 {
            match node {
                BootNode::Enr(enr) => {
                    if let Err(e) = self.disc.add_enr(enr.clone()) {
                        warn!(target: "p2p::discv5::driver", "Failed to bootstrap discovery service: {:?}", e);
                        continue;
                    }
                }
                BootNode::Enode(enode) => {
                    if let Err(err) = self.disc.request_enr(enode.to_string()).await {
                        debug!(target: "p2p::discv5",
                            ?enode,
                            %err,
                            "failed adding boot node"
                        );
                    }
                }
            }
        }
    }

    /// Spawns a new [`Discv5`] discovery service in a new tokio task.
    ///
    /// Returns a [`Discv5Handler`] to communicate with the spawned task.
    pub fn start(mut self) -> Discv5Handler {
        let (req_sender, mut req_recv) = channel::<HandlerRequest>(1024);
        let (res_sender, res_recv) = channel::<HandlerResponse>(1024);
        let (enr_sender, enr_recv) = channel::<Enr>(1024);
        let (_events_sender, events_recv) = channel::<Event>(1024);

        tokio::spawn(async move {
            // Step 1: Start the discovery service.
            self.init().await;
            info!(target: "p2p::discv5::driver", "Started `Discv5` peer discovery");

            // Step 2: Bootstrap discovery service bootnodes.
            self.bootstrap().await;
            info!(target: "p2p::discv5::driver", "Bootstrapped `Discv5` bootnodes");

            // Interval to find new nodes.
            let mut interval = tokio::time::interval(self.interval);

            // Step 3: Run the core driver loop.
            loop {
                tokio::select! {
                    msg = req_recv.recv() => {
                        match msg {
                            Some(msg) => match msg {
                                HandlerRequest::Metrics => {
                                    let metrics = self.disc.metrics();
                                    let _ = res_sender.send(HandlerResponse::Metrics(metrics)).await;
                                }
                                HandlerRequest::PeerCount => {
                                    let peers = self.disc.connected_peers();
                                    let _ = res_sender.send(HandlerResponse::PeerCount(peers)).await;
                                }
                                HandlerRequest::LocalEnr => {
                                    let enr = self.disc.local_enr().clone();
                                    let _ = res_sender.send(HandlerResponse::LocalEnr(enr)).await;
                                }
                                HandlerRequest::AddEnr(enr) => {
                                    let _ = self.disc.add_enr(enr);
                                }
                                HandlerRequest::RequestEnr(enode) => {
                                    let _ = self.disc.request_enr(enode).await;
                                }
                                HandlerRequest::TableEnrs => {
                                    let enrs = self.disc.table_entries_enr();
                                    let _ = res_sender.send(HandlerResponse::TableEnrs(enrs)).await;
                                }
                            }
                            None => {
                                trace!(target: "p2p::discv5::driver", "Receiver `None` peer enr");
                            }
                        }
                    }
                    // Receive an event from the event stream.
                    // Forward it along to the handler.
                    // receiver = self.disc.event_stream() => {
                    //     let Ok(mut recv) = receiver else {
                    //         warn!(target: "p2p::discv5::driver", "Failed to receive event stream");
                    //         continue;
                    //     };
                    //     let Some(event) = recv.recv().await else {
                    //         trace!(target: "p2p::discv5::driver", "Empty event received");
                    //         continue;
                    //     };
                    //     // It's up to the handler to consume events.
                    //     // Don't block the driver loop.
                    //     if let Err(e) = events_sender.try_send(event) {
                    //         trace!(target: "p2p::discv5::driver", "Failed to forward event: {:?}", e);
                    //     }
                    // }
                    _ = interval.tick() => {
                        info!(target: "p2p::discv5::driver", "Finding new nodes...");
                        match self.disc.find_node(NodeId::random()).await {
                            Ok(nodes) => {
                                let enrs =
                                    nodes.iter().filter(|node| OpStackEnr::is_valid_node(node, self.chain_id));

                                for enr in enrs {
                                    _ = enr_sender.send(enr.clone()).await;
                                }
                            }
                            Err(err) => {
                                warn!(target: "p2p::discv5::driver", "discovery error: {:?}", err);
                            }
                        }
                    }
                }
            }
        });

        Discv5Handler::new(req_sender, res_recv, events_recv, enr_recv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[tokio::test]
    async fn test_discv5_driver() {
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9099);
        let discovery = Discv5Driver::builder()
            .with_address(socket)
            .with_chain_id(10)
            .build()
            .expect("Failed to build discovery service");
        let _ = discovery.start();
        // The service starts.
        // TODO: verify with a heartbeat.
    }
}
