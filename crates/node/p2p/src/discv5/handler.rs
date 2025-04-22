//! Handler to the [`discv5::Discv5`] service spawned in a thread.

use discv5::{Enr, metrics::Metrics};
use libp2p::Multiaddr;
use std::{collections::HashSet, string::String, sync::Arc, time::Duration};
use tokio::sync::mpsc::Sender;

/// A request from the [`Discv5Handler`] to the spawned [`discv5::Discv5`] service.
#[derive(Debug)]
pub enum HandlerRequest {
    /// Requests [`Metrics`] from the [`discv5::Discv5`] service.
    Metrics(tokio::sync::oneshot::Sender<Metrics>),
    /// Returns the number of connected peers.
    PeerCount(tokio::sync::oneshot::Sender<usize>),
    /// Request for the [`discv5::Discv5`] service to call [`discv5::Discv5::add_enr`] with the
    /// specified [`Enr`].
    AddEnr(Enr),
    /// Requests for the [`discv5::Discv5`] service to call [`discv5::Discv5::request_enr`] with the
    /// specified string.
    RequestEnr(String),
    /// Requests the local [`Enr`].
    LocalEnr(tokio::sync::oneshot::Sender<Enr>),
    /// Requests the table ENRs.
    TableEnrs(tokio::sync::oneshot::Sender<Vec<Enr>>),
    /// Bans the nodes matching the given set of [`Multiaddr`] for the given duration.
    BanAddrs { addrs_to_ban: Arc<HashSet<Multiaddr>>, ban_duration: Duration },
}

/// Handler to the spawned [`discv5::Discv5`] service.
///
/// Provides a lock-free way to access the spawned `discv5::Discv5` service
/// by using message-passing to relay requests and responses through
/// a channel.
#[derive(Debug, Clone)]
pub struct Discv5Handler {
    /// Sends [`HandlerRequest`]s to the spawned [`discv5::Discv5`] service.
    pub sender: Sender<HandlerRequest>,
    /// The chain id.
    pub chain_id: u64,
}

impl Discv5Handler {
    /// Creates a new [`Discv5Handler`] service.
    pub fn new(chain_id: u64, sender: Sender<HandlerRequest>) -> Self {
        Self { sender, chain_id }
    }

    /// Blocking request for the ENRs of the discovery service.
    ///
    /// Returns `None` if the request could not be sent or received.
    pub fn table_enrs(&self) -> tokio::sync::oneshot::Receiver<Vec<Enr>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            if let Err(e) = sender.send(HandlerRequest::TableEnrs(tx)).await {
                warn!("Failed to send table ENRs request: {:?}", e);
            }
        });
        rx
    }

    /// Blocking request for the local ENR of the node.
    ///
    /// Returns `None` if the request could not be sent or received.
    pub fn local_enr(&self) -> tokio::sync::oneshot::Receiver<Enr> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            if let Err(e) = sender.send(HandlerRequest::LocalEnr(tx)).await {
                warn!("Failed to send local ENR request: {:?}", e);
            }
        });
        rx
    }

    /// Blocking request for the metrics of the discovery service.
    ///
    /// Returns `None` if the request could not be sent or received.
    pub fn metrics(&self) -> tokio::sync::oneshot::Receiver<Metrics> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            if let Err(e) = sender.send(HandlerRequest::Metrics(tx)).await {
                warn!("Failed to send metrics request: {:?}", e);
            }
        });
        rx
    }

    /// Blocking request for the discovery service peer count.
    ///
    /// Returns `None` if the request could not be sent or received.
    pub fn peer_count(&self) -> tokio::sync::oneshot::Receiver<usize> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            if let Err(e) = sender.send(HandlerRequest::PeerCount(tx)).await {
                warn!("Failed to send peer count request: {:?}", e);
            }
        });
        rx
    }
}
