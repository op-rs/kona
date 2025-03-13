//! Contains the `AnyNode` enum, which can represent a peer in any form.

use crate::{NodeRecord, PeerId};
use discv5::{Enr, enr::EnrPublicKey};

/// A peer that can come in [`Enr`] or [`NodeRecord`] form.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum AnyNode {
    /// An "enode:" peer with full ip
    NodeRecord(NodeRecord),
    /// An "enr:" peer
    Enr(Enr),
    /// An incomplete "enode" with only a peer id
    PeerId(PeerId),
}

impl AnyNode {
    /// Returns the peer id of the node.
    pub fn peer_id(&self) -> PeerId {
        match self {
            Self::NodeRecord(record) => record.id,
            Self::Enr(enr) => {
                PeerId::from_slice(&enr.public_key().encode_uncompressed().to_vec()[1..])
            }
            Self::PeerId(peer_id) => *peer_id,
        }
    }
}
