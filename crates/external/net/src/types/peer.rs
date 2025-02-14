//! Peer Types

use discv5::enr::{CombinedKey, Enr};
use libp2p::{multiaddr::Protocol, Multiaddr};
use std::net::{IpAddr, SocketAddr};

/// A wrapper around a peer's [SocketAddr].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Peer {
    /// The peer's [SocketAddr].
    pub socket: SocketAddr,
}

/// An error that can occur when converting types to a [Peer].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PeerConversionError {
    /// The IP Address is missing.
    #[error("IP address is missing")]
    MissingIp,
    /// The port is missing.
    #[error("port is missing")]
    MissingPort,
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Peer {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.arbitrary::<bool>()? {
            true => {
                let ipv6 = u.arbitrary::<[u8; 16]>()?;
                let port = u.arbitrary::<u16>()?;
                Ok(Peer { socket: SocketAddr::new(IpAddr::V6(ipv6.into()), port) })
            }
            false => {
                let ipv4 = u.arbitrary::<u8>()?;
                let port = u.arbitrary::<u16>()?;
                Ok(Peer { socket: SocketAddr::new(IpAddr::V4([ipv4; 4].into()), port) })
            }
        }
    }
}

impl TryFrom<&Enr<CombinedKey>> for Peer {
    type Error = PeerConversionError;

    /// Converts an [Enr] to a Peer
    fn try_from(value: &Enr<CombinedKey>) -> Result<Self, PeerConversionError> {
        let ip = value.ip4().ok_or(PeerConversionError::MissingIp)?;
        let port = value.tcp4().ok_or(PeerConversionError::MissingPort)?;
        let socket = SocketAddr::new(IpAddr::V4(ip), port);
        Ok(Peer { socket })
    }
}

impl From<Peer> for Multiaddr {
    /// Converts a Peer to a [Multiaddr]
    fn from(value: Peer) -> Self {
        let mut multiaddr = Multiaddr::empty();
        match value.socket.ip() {
            IpAddr::V4(ip) => multiaddr.push(Protocol::Ip4(ip)),
            IpAddr::V6(ip) => multiaddr.push(Protocol::Ip6(ip)),
        }
        multiaddr.push(Protocol::Tcp(value.socket.port()));
        multiaddr
    }
}

impl TryFrom<&Multiaddr> for Peer {
    type Error = PeerConversionError;

    /// Converts a [Multiaddr] to a Peer
    fn try_from(value: &Multiaddr) -> Result<Self, PeerConversionError> {
        let mut ip = None;
        let mut port = None;
        for protocol in value.iter() {
            match protocol {
                Protocol::Ip4(ip4) => {
                    ip = Some(IpAddr::V4(ip4));
                }
                Protocol::Ip6(ip6) => {
                    ip = Some(IpAddr::V6(ip6));
                }
                Protocol::Tcp(tcp) => {
                    port = Some(tcp);
                }
                _ => {}
            }
        }
        let ip = ip.ok_or(PeerConversionError::MissingIp)?;
        let port = port.ok_or(PeerConversionError::MissingPort)?;
        Ok(Peer { socket: SocketAddr::new(ip, port) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "arbitrary")]
    fn test_peer_to_multiaddr() {
        arbtest::arbtest(|u| {
            use arbitrary::Arbitrary;
            let peer = Peer::arbitrary(u)?;
            let mut multiaddr = Multiaddr::from(peer.clone());
            // This will be ignored since only the ipv4, ipv6, and tcp protocols are supported.
            multiaddr.push(libp2p::multiaddr::Protocol::Dccp(10));
            let peer2 =
                Peer::try_from(&multiaddr).map_err(|_| arbitrary::Error::IncorrectFormat)?;
            assert_eq!(peer, peer2);
            Ok(())
        });
    }

    #[test]
    fn test_peer_from_enr_without_ip() {
        let key = CombinedKey::generate_secp256k1();
        let enr = Enr::<CombinedKey>::builder().build(&key).unwrap();
        let err = Peer::try_from(&enr).unwrap_err();
        assert_eq!(err, PeerConversionError::MissingIp);
    }

    #[test]
    fn test_peer_from_enr_without_port() {
        let key = CombinedKey::generate_secp256k1();
        let ip = std::net::Ipv4Addr::new(192, 168, 0, 1);
        let enr = Enr::<CombinedKey>::builder().ip4(ip).build(&key).unwrap();
        let err = Peer::try_from(&enr).unwrap_err();
        assert_eq!(err, PeerConversionError::MissingPort);
    }

    #[test]
    fn test_peer_from_enr_succeeds() {
        let key = CombinedKey::generate_secp256k1();
        let ip = std::net::Ipv4Addr::new(192, 168, 0, 1);
        let port = 30303;
        let enr = Enr::<CombinedKey>::builder().ip4(ip).tcp4(port).build(&key).unwrap();
        let peer = Peer::try_from(&enr).unwrap();
        assert_eq!(peer.socket, SocketAddr::new(IpAddr::V4(ip), port));
    }
}
