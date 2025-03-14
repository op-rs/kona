//! Handler to the [`discv5::Discv5`] service spawned in a thread.

use discv5::{Enr, metrics::Metrics};
use std::string::String;
use tokio::sync::mpsc::{Receiver, Sender};

/// A request from the [`Discv5Handler`] to the spawned [`discv5::Discv5`] service.
#[derive(Debug, Clone)]
pub enum HandlerRequest {
    /// Requests [`Metrics`] from the [`discv5::Discv5`] service.
    Metrics,
    /// Returns the number of connected peers.
    PeerCount,
    /// Request for the [`discv5::Discv5`] service to call [`discv5::Discv5::add_enr`] with the
    /// specified [`Enr`].
    AddEnr(Enr),
    /// Requests for the [`discv5::Discv5`] service to call [`discv5::Discv5::request_enr`] with the
    /// specified string.
    RequestEnr(String),
    /// Requests the local [`Enr`].
    LocalEnr,
}

/// A response from the spawned [`discv5::Discv5`] service thread to the [`Discv5Handler`].
#[derive(Debug, Clone)]
pub enum HandlerResponse {
    /// Requests [`Metrics`] from the [`discv5::Discv5`] service.
    Metrics(Metrics),
    /// Returns the number of connected peers.
    PeerCount(usize),
    /// Requests the local [`Enr`].
    LocalEnr(Enr),
}

/// Handler to the spawned [`discv5::Discv5`] service.
///
/// Provides a lock-free way to access the spawned `discv5::Discv5` service
/// by using message-passing to relay requests and responses through
/// a channel.
pub struct Discv5Handler {
    /// Sends [`HandlerRequest`]s to the spawned [`discv5::Discv5`] service.
    pub sender: Sender<HandlerRequest>,
    /// Receives [`HandlerResponse`]s from the spawned [`discv5::Discv5`] service.
    pub receiver: Receiver<HandlerResponse>,
    /// Receives new [`Enr`]s.
    pub enr_receiver: Receiver<Enr>,
}

impl Discv5Handler {
    /// Creates a new [`Discv5Handler`] service.
    pub fn new(
        sender: Sender<HandlerRequest>,
        receiver: Receiver<HandlerResponse>,
        enr_receiver: Receiver<Enr>,
    ) -> Self {
        Self { sender, receiver, enr_receiver }
    }

    /// Returns the local ENR of the node.
    pub async fn local_enr(&mut self) -> Option<Enr> {
        let _ = self.sender.send(HandlerRequest::LocalEnr).await;
        match self.receiver.recv().await {
            Some(HandlerResponse::LocalEnr(enr)) => Some(enr),
            _ => None,
        }
    }

    /// Gets metrics for the discovery service.
    pub async fn metrics(&mut self) -> Option<Metrics> {
        let _ = self.sender.send(HandlerRequest::Metrics).await;
        match self.receiver.recv().await {
            Some(HandlerResponse::Metrics(metrics)) => Some(metrics),
            _ => None,
        }
    }

    /// Returns the number of connected peers.
    pub async fn peers(&mut self) -> Option<usize> {
        let _ = self.sender.send(HandlerRequest::PeerCount).await;
        match self.receiver.recv().await {
            Some(HandlerResponse::PeerCount(count)) => Some(count),
            _ => None,
        }
    }
}
