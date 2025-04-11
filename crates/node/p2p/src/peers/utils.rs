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

/// Converts a [`Multiaddr`] into an [`Enr`].
pub fn multiaddr_to_enr(_addr: &Multiaddr) -> Option<Enr> {
    // let mut builder = Enr::builder();
    // for protocol in addr.iter() {
    //     match protocol {
    //         Protocol::Ip4(ip) => _ = builder.ip4(ip),
    //         Protocol::Ip6(ip) => _ = builder.ip6(ip),
    //         Protocol::Tcp(port) => {
    //             builder.tcp4(port);
    //             builder.tcp6(port);
    //         }
    //         Protocol::Udp(port) => {
    //             builder.udp4(port);
    //             builder.udp6(port);
    //         }
    //         _ => {}
    //     }
    // }
    // builder.build().ok()
    None
}
