//! Contains the [`BootNode`] type which is used to represent a boot node in the network.

use std::net::IpAddr;
use discv5::{multiaddr::{Protocol, Multiaddr}, Enr};
use crate::NodeRecord;

/// A boot node can be added either as a string in either 'enode' URL scheme or serialized from
/// [`Enr`] type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Display)]
pub enum BootNode {
    /// An unsigned node record.
    #[display("{_0}")]
    Enode(Multiaddr),
    /// A signed node record.
    #[display("{_0:?}")]
    Enr(Enr),
}

/// Converts a [`PeerId`] to a [`discv5::libp2p_identity::PeerId`].
pub fn discv4_id_to_multiaddr_id(
    peer_id: PeerId,
) -> Result<discv5::libp2p_identity::PeerId, secp256k1::Error> {
    let pk = id2pk(peer_id)?.encode();
    let pk: discv5::libp2p_identity::PublicKey =
        discv5::libp2p_identity::secp256k1::PublicKey::try_from_bytes(&pk).unwrap().into();

    Ok(pk.to_peer_id())
}

impl BootNode {
    /// Parses a [`NodeRecord`] and serializes according to CL format. Note: [`discv5`] is
    /// originally a CL library hence needs this format to add the node.
    pub fn from_unsigned(node_record: NodeRecord) -> Result<Self, secp256k1::Error> {
        let NodeRecord { address, udp_port, id, .. } = node_record;
        let mut multi_address = Multiaddr::empty();
        match address {
            IpAddr::V4(ip) => multi_address.push(Protocol::Ip4(ip)),
            IpAddr::V6(ip) => multi_address.push(Protocol::Ip6(ip)),
        }

        multi_address.push(Protocol::Udp(udp_port));
        let id = discv4_id_to_multiaddr_id(id)?;
        multi_address.push(Protocol::P2p(id));

        Ok(Self::Enode(multi_address))
    }
}
