//! Utilities to translate types.

use discv5::{Enr, multiaddr::Protocol};
use libp2p::Multiaddr;

/// Converts an [`Enr`] into a [`Multiaddr`].
pub fn enr_to_multiaddr(enr: &Enr) -> Option<Multiaddr> {
    if let Some(socket) = enr.tcp4_socket() {
        let mut addr = Multiaddr::from(*socket.ip());
        addr.push(Protocol::Tcp(socket.port()));
        return Some(addr);
    }
    if let Some(socket) = enr.tcp6_socket() {
        let mut addr = Multiaddr::from(*socket.ip());
        addr.push(Protocol::Tcp(socket.port()));
        return Some(addr)
    }
    None
}
