//! Utilities to translate types.

use discv5::{Enr, multiaddr::Protocol};
use libp2p::Multiaddr;

/// Converts an [`Enr`] into a [`Multiaddr`].
pub fn enr_to_multiaddr(enr: &Enr) -> Option<Multiaddr> {
    use discv5::enr::EnrPublicKey;
    use libp2p::PeerId;
    let peer_id_slice = &enr.public_key().encode_uncompressed();
    let prefixed = [&[0x12, 0x40], peer_id_slice.as_slice()].concat();
    let hash = libp2p::multihash::Multihash::<64>::from_bytes(&prefixed).unwrap();
    let libp2p_id = PeerId::from_multihash(hash).ok()?;
    if let Some(socket) = enr.tcp4_socket() {
        let mut addr = Multiaddr::from(*socket.ip());
        addr.push(Protocol::Tcp(socket.port()));
        addr.push(Protocol::P2p(libp2p_id));
        return Some(addr);
    }
    if let Some(socket) = enr.tcp6_socket() {
        let mut addr = Multiaddr::from(*socket.ip());
        addr.push(Protocol::Tcp(socket.port()));
        addr.push(Protocol::P2p(libp2p_id));
        return Some(addr)
    }
    None
}
