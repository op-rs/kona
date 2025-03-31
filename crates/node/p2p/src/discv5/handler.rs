//! Handler to the [`discv5::Discv5`] service spawned in a thread.

use discv5::{Enr, Event, metrics::Metrics};
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
    /// Requests the table ENRs.
    TableEnrs,
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
    /// Table Enrs
    TableEnrs(Vec<Enr>),
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
    /// [`Event`] receiver.
    pub events: Receiver<Event>,
    /// Receives new [`Enr`]s.
    pub enr_receiver: Receiver<Enr>,
    /// The chain id.
    pub chain_id: u64,
}

impl Discv5Handler {
    /// Creates a new [`Discv5Handler`] service.
    pub fn new(
        chain_id: u64,
        sender: Sender<HandlerRequest>,
        receiver: Receiver<HandlerResponse>,
        events: Receiver<Event>,
        enr_receiver: Receiver<Enr>,
    ) -> Self {
        Self { sender, receiver, events, enr_receiver, chain_id }
    }

    /// Receives an [`Event`] from the discovery service.
    pub async fn event(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    /// Returns the local ENR of the node.
    pub async fn local_enr(&mut self) -> Option<Enr> {
        let _ = self.sender.send(HandlerRequest::LocalEnr).await;
        match self.receiver.recv().await {
            Some(HandlerResponse::LocalEnr(enr)) => Some(enr),
            _ => None,
        }
    }

    /// Requests [`Enr`]s from the discovery service.
    pub async fn table_enrs(&mut self) -> Vec<Enr> {
        let _ = self.sender.send(HandlerRequest::TableEnrs).await;
        match self.receiver.recv().await {
            Some(HandlerResponse::TableEnrs(enrs)) => enrs,
            _ => vec![],
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

    /// Non-blocking request for the discovery service peer count.
    ///
    /// Returns `None` if the request could not be sent or received.
    pub fn peers(&mut self) -> Option<usize> {
        if self.sender.try_send(HandlerRequest::PeerCount).is_err() {
            return None;
        }
        match self.receiver.try_recv() {
            Ok(HandlerResponse::PeerCount(count)) => Some(count),
            _ => None,
        }
    }
}
