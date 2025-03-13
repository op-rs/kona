//! Contains the [`BootNode`] type which is used to represent a boot node in the network.

use crate::NodeRecord;
use derive_more::{Display, From};
use discv5::{
    Enr,
    multiaddr::{Multiaddr, PeerId, Protocol},
};
use std::net::IpAddr;

/// A boot node can be added either as a string in either 'enode' URL scheme or serialized from
/// [`Enr`] type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, From, Display)]
pub enum BootNode {
    /// An unsigned node record.
    #[display("{_0}")]
    Enode(Multiaddr),
    /// A signed node record.
    #[display("{_0:?}")]
    Enr(Enr),
}

impl BootNode {
    /// Parses a [`NodeRecord`] and serializes according to CL format. Note: [`discv5`] is
    /// originally a CL library hence needs this format to add the node.
    pub fn from_unsigned(
        node_record: NodeRecord,
    ) -> Result<Self, discv5::libp2p_identity::ParseError> {
        let NodeRecord { address, udp_port, id, .. } = node_record;
        let mut multi_address = Multiaddr::empty();
        match address {
            IpAddr::V4(ip) => multi_address.push(Protocol::Ip4(ip)),
            IpAddr::V6(ip) => multi_address.push(Protocol::Ip6(ip)),
        }

        multi_address.push(Protocol::Udp(udp_port));
        let slice_peer_id = [&[0x12, 0x40], id.as_slice()].concat();
        multi_address.push(Protocol::P2p(PeerId::from_bytes(&slice_peer_id)?));

        Ok(Self::Enode(multi_address))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_local_peer_id_to_multi_peer_id() {
        let local_peer_id = crate::PeerId::ZERO;
        assert_eq!(local_peer_id.len(), 64);
        let slice_peer_id = [&[0x12, 0x40], local_peer_id.as_slice()].concat();
        let hash = multihash::Multihash::<64>::from_bytes(&slice_peer_id).unwrap();
        let _ = PeerId::from_multihash(hash).unwrap();
    }
}
