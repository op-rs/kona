//! Wrapper around the Discv5 service.

use discv5::Discv5;
use discv5::enr::NodeId;
use std::sync::Arc;
use thiserror::Error;
use std::collections::HashSet;
use crate::BootNode;

/// An error that bubbled up from the [`Discv5`] service.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Discv5WrapperError {
    /// A query error when finding a node.
    #[error(transparent)]
    Query(#[from] discv5::QueryError),
}

/// Wraps the [`Discv5`] service, providing a more ergonomic API.
#[derive(Clone)]
pub struct Discv5Wrapper(pub Arc<Discv5>);

impl Discv5Wrapper {
    /// Creates a new Discv5 service.
    pub fn new(config: Config) -> Self {
        let disc = Discv5::new(config);
        Self(Arc::new(disc))
    }

    /// Returns a new [`Arc`] reference to the underlying [`Discv5`] service.
    pub fn inner(&self) -> Arc<Discv5> {
        self.0.clone()
    }

    /// Starts the discovery service.
    pub async fn start(&self) -> Result<()> {
        self.0.start().await
    }

    /// Finds the node with the given target.
    pub async fn find_node(&self) -> Result<Vec<Enr>, Discv5Error> {
        self.0.find_node(NodeId::random()).await
    }

    /// Bootstraps underlying [`discv5::Discv5`] node with configured peers.
    ///
    /// Adapted from <https://github.com/paradigmxyz/reth/blob/main/crates/net/discv5/src/lib.rs>.
    pub async fn bootstrap(&self, nodes: HashSet<BootNode>) -> Result<(), Error> {
        trace!(target: "net::discv5",
            ?nodes,
            "adding bootstrap nodes .."
        );

        let mut enr_requests = vec![];
        for node in nodes {
            match node {
                BootNode::Enr(node) => {
                    if let Err(err) = discv5.add_enr(node) {
                        return Err(Error::AddNodeFailed(err))
                    }
                }
                BootNode::Enode(enode) => {
                    let discv5 = self.inner();
                    enr_requests.push(async move {
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
