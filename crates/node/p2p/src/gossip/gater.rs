//! An implementation of the [`ConnectionGate`] trait.

use crate::ConnectionGate;
use libp2p::{Multiaddr, PeerId};
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
};

/// Connection Gater
///
/// A connection gate that regulates peer connections for the libp2p gossip swarm.
///
/// An implementation of the [`ConnectionGate`] trait.
#[derive(Default, Debug, Clone)]
pub struct ConnectionGater {
    /// The number of times to redial a peer.
    pub peer_redialing: Option<u64>,
    /// A set of [`PeerId`]s that are currently being dialed.
    pub current_dials: HashSet<PeerId>,
    /// A mapping from [`Multiaddr`] to the number of times it has been dialed.
    ///
    /// A peer cannot be redialed more than [`GossipDriverBuilder.peer_redialing`] times.
    pub dialed_peers: HashMap<Multiaddr, u64>,
    /// A set of protected peers that cannot be disconnected.
    ///
    /// Protecting a peer prevents the peer from any redial thresholds or peer scoring.
    pub protected_peers: HashSet<Multiaddr>,
    /// A set of blocked peer ids.
    pub blocked_peers: HashSet<PeerId>,
    /// A set of blocked ip addresses that cannot be dialed.
    pub blocked_addrs: HashSet<IpAddr>,
}

impl ConnectionGater {
    /// Creates a new instance of the `ConnectionGater`.
    pub fn new(peer_redialing: Option<u64>) -> Self {
        Self {
            peer_redialing,
            current_dials: HashSet::new(),
            dialed_peers: HashMap::new(),
            protected_peers: HashSet::new(),
            blocked_peers: HashSet::new(),
            blocked_addrs: HashSet::new(),
        }
    }

    /// Returns if the given [`Multiaddr`] has been dialed the maximum number of times.
    pub fn dial_threshold_reached(&self, addr: &Multiaddr) -> bool {
        // If the peer has not been dialed yet, the threshold is not reached.
        let Some(dialed) = self.dialed_peers.get(addr) else {
            return false;
        };
        // If the peer has been dialed and the threshold is not set, the threshold is reached.
        let Some(redialing) = self.peer_redialing else {
            return true;
        };
        // If the threshold is set to `0`, redial indefinitely.
        if redialing == 0 {
            return false;
        }
        if *dialed >= redialing {
            return true;
        }
        false
    }

    /// Gets the [`PeerId`] from a given [`Multiaddr`].
    pub fn peer_id_from_addr(addr: &Multiaddr) -> Option<PeerId> {
        addr.iter().find_map(|component| match component {
            libp2p::multiaddr::Protocol::P2p(peer_id) => Some(peer_id),
            _ => None,
        })
    }

    /// Constructs the [`IpAddr`] from the given [`Multiaddr`].
    pub fn ip_from_addr(addr: &Multiaddr) -> Option<IpAddr> {
        addr.iter().find_map(|component| match component {
            libp2p::multiaddr::Protocol::Ip4(ip) => Some(IpAddr::V4(ip)),
            libp2p::multiaddr::Protocol::Ip6(ip) => Some(IpAddr::V6(ip)),
            _ => None,
        })
    }
}

impl ConnectionGate for ConnectionGater {
    fn can_dial(&self, addr: &Multiaddr) -> bool {
        // Get the peer id from the given multiaddr.
        let Some(peer_id) = Self::peer_id_from_addr(addr) else {
            warn!(target: "p2p", peer=?addr, "Failed to extract PeerId from Multiaddr");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "invalid_multiaddr");
            return false;
        };

        // Cannot dial a peer that is already being dialed.
        if self.current_dials.contains(&peer_id) {
            debug!(target: "gossip", peer=?addr, "Already dialing peer, not dialing");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "already_dialing", "peer" => peer_id.to_string());
            return false;
        }

        // If the peer is protected, do not apply thresholds.
        let protected = self.protected_peers.contains(addr);

        // If the peer is not protected, and its dial threshold is reached, do not dial.
        if !protected && self.dial_threshold_reached(addr) {
            debug!(target: "gossip", peer=?addr, "Dial threshold reached, not dialing");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "threshold_reached", "peer" => peer_id.to_string());
            return false;
        }

        // If the peer is blocked, do not dial.
        if self.blocked_peers.contains(&peer_id) {
            debug!(target: "gossip", peer=?addr, "Peer is blocked, not dialing");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "blocked_peer", "peer" => peer_id.to_string());
            return false;
        }

        // There must be a reachable IP Address in the Multiaddr protocol stack.
        let Some(ip_addr) = Self::ip_from_addr(addr) else {
            warn!(target: "p2p", peer=?addr, "Failed to extract IpAddr from Multiaddr");
            return false;
        };

        // If the address is blocked, do not dial.
        if self.blocked_addrs.contains(&ip_addr) {
            debug!(target: "gossip", peer=?addr, "Address is blocked, not dialing");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "blocked_address", "peer" => peer_id.to_string());
            return false;
        }

        true
    }

    fn dialing(&mut self, addr: &Multiaddr) {
        if let Some(peer_id) = Self::peer_id_from_addr(addr) {
            self.current_dials.insert(peer_id);
        } else {
            warn!(target: "p2p", peer=?addr, "Failed to extract PeerId from Multiaddr when dialing");
        }
    }

    fn dialed(&mut self, addr: &Multiaddr) {
        let count = self.dialed_peers.entry(addr.clone()).or_insert(0);
        *count += 1;
        trace!(target: "gossip", peer=?addr, "Dialed peer, current count: {}", count);
    }

    fn remove_dial(&mut self, peer_id: &PeerId) {
        self.current_dials.remove(peer_id);
    }

    fn can_disconnect(&self, peer_id: &Multiaddr) -> bool {
        // If the peer is protected, do not disconnect.
        if self.protected_peers.contains(peer_id) {
            debug!(target: "gossip", peer=?peer_id, "Peer is protected, cannot disconnect");
            return false;
        }
        true
    }

    fn block_peer(&mut self, peer_id: &PeerId) {
        self.blocked_peers.insert(*peer_id);
        debug!(target: "gossip", peer=?peer_id, "Blocked peer");
    }

    fn unblock_peer(&mut self, peer_id: &PeerId) {
        self.blocked_peers.remove(peer_id);
        debug!(target: "gossip", peer=?peer_id, "Unblocked peer");
    }

    fn list_blocked_peers(&self) -> Vec<PeerId> {
        self.blocked_peers.iter().copied().collect()
    }

    fn block_addr(&mut self, ip: IpAddr) {
        self.blocked_addrs.insert(ip);
        debug!(target: "gossip", ?ip, "Blocked ip address");
    }

    fn unblock_addr(&mut self, ip: IpAddr) {
        self.blocked_addrs.remove(&ip);
        debug!(target: "gossip", ?ip, "Unblocked ip address");
    }

    fn list_blocked_addrs(&self) -> Vec<IpAddr> {
        self.blocked_addrs.iter().cloned().collect()
    }

    fn block_subnet(&mut self, _subnet: &str) {
        // TODO
    }

    fn unblock_subnet(&mut self, _subnet: &str) {
        // TODO
    }

    fn list_blocked_subnets(&self) -> Vec<String> {
        // TODO
        vec![]
    }

    fn protect_peer(&mut self, peer_id: &Multiaddr) {
        self.protected_peers.insert(peer_id.clone());
        debug!(target: "gossip", peer=?peer_id, "Protected peer");
    }

    fn unprotect_peer(&mut self, peer_id: &Multiaddr) {
        self.protected_peers.remove(peer_id);
        debug!(target: "gossip", peer=?peer_id, "Unprotected peer");
    }
}
