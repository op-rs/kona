//! Wrapper around the Discv5 service.

use crate::BootNode;
use discv5::{Discv5, Enr, enr::NodeId};
use futures::future::join_all;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

/// An error that bubbled up from the [`Discv5`] service.
#[derive(Error, Debug)]
pub enum Discv5WrapperError {
    /// A query error when finding a node.
    #[error("query error: {0}")]
    Query(discv5::QueryError),
    /// An error when adding a node.
    #[error("add node failed: {0}")]
    AddNodeFailed(&'static str),
    /// A [`Discv5`] error.
    #[error("discv5 error: {0}")]
    Discv5(discv5::Error),
}

/// Wraps the [`Discv5`] service, providing a more ergonomic API.
#[derive(Clone)]
pub struct Discv5Wrapper(pub Arc<Mutex<Discv5>>);

impl Discv5Wrapper {
    /// Creates a new Discv5 service.
    pub fn new(inner: Discv5) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }

    /// Returns a new [`Arc`] reference to the underlying [`Discv5`] service.
    pub fn inner(&self) -> Arc<Mutex<Discv5>> {
        self.0.clone()
    }

    /// Returns the local ENR of the node.
    pub async fn local_enr(&self) -> Enr {
        self.0.lock().await.local_enr().clone()
    }

    /// Starts the discovery service.
    pub async fn start(&mut self) -> Result<(), Discv5WrapperError> {
        let mut discv5 = self.0.lock().await;
        discv5.start().await.map_err(Discv5WrapperError::Discv5)
    }

    /// Finds the node with the given target.
    pub async fn find_node(&self) -> Result<Vec<Enr>, Discv5WrapperError> {
        let discv5 = self.0.lock().await;
        discv5.find_node(NodeId::random()).await.map_err(Discv5WrapperError::Query)
    }

    /// Gets metrics for the discovery service.
    pub async fn metrics(&self) -> discv5::metrics::Metrics {
        let discv5 = self.0.lock().await;
        discv5.metrics()
    }

    /// Returns the number of connected peers.
    pub async fn peers(&self) -> usize {
        let discv5 = self.0.lock().await;
        discv5.connected_peers()
    }

    /// Bootstraps underlying [`discv5::Discv5`] node with configured peers.
    pub async fn bootstrap(&self, nodes: &[BootNode]) -> Result<(), Discv5WrapperError> {
        trace!(target: "net::discv5",
            ?nodes,
            "adding bootstrap nodes .."
        );

        let mut enr_requests = vec![];
        for node in nodes {
            match node {
                BootNode::Enr(enr) => {
                    let discv5 = self.0.lock().await;
                    discv5.add_enr(enr.clone()).map_err(Discv5WrapperError::AddNodeFailed)?;
                }
                BootNode::Enode(enode) => {
                    let discv5 = self.0.clone();
                    enr_requests.push(async move {
                        let discv5 = discv5.lock().await;
                        if let Err(err) = discv5.request_enr(enode.to_string()).await {
                            debug!(target: "p2p::discv5",
                                ?enode,
                                %err,
                                "failed adding boot node"
                            );
                        }
                    })
                }
            }
        }

        // If a session is established, the ENR is added straight away to discv5 kbuckets
        Ok(_ = join_all(enr_requests).await)
    }
}
