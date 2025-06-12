//! An implementation of the [`ConnectionGate`] trait.

use crate::{Connectedness, ConnectionGate};
use ipnet::IpNet;
use libp2p::{Multiaddr, PeerId};
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    time::Duration,
};
use tokio::time::Instant;

/// Dial info for a peer.
#[derive(Debug, Clone)]
pub struct DialInfo {
    /// Number of times the peer has been dialed during the current dial period.
    /// This number is reset once the last time the peer was dialed is longer than the dial period.
    pub num_dials: u64,
    /// The last time the peer was dialed.
    pub last_dial: Instant,
}

impl Default for DialInfo {
    fn default() -> Self {
        Self { num_dials: 0, last_dial: Instant::now() }
    }
}

/// Connection Gater
///
/// A connection gate that regulates peer connections for the libp2p gossip swarm.
///
/// An implementation of the [`ConnectionGate`] trait.
#[derive(Default, Debug, Clone)]
pub struct ConnectionGater {
    /// The number of times to dial a peer.
    pub peer_redialing: Option<u64>,
    /// A set of [`PeerId`]s that are currently being dialed.
    pub current_dials: HashSet<PeerId>,
    /// A mapping from [`Multiaddr`] to the dial info for the peer.
    pub dialed_peers: HashMap<Multiaddr, DialInfo>,
    /// Holds a map from peer id to connectedness for the given peer id.
    pub connectedness: HashMap<PeerId, Connectedness>,
    /// A set of protected peers that cannot be disconnected.
    ///
    /// Protecting a peer prevents the peer from any redial thresholds or peer scoring.
    pub protected_peers: HashSet<PeerId>,
    /// A set of blocked peer ids.
    pub blocked_peers: HashSet<PeerId>,
    /// A set of blocked ip addresses that cannot be dialed.
    pub blocked_addrs: HashSet<IpAddr>,
    /// A set of blocked subnets that cannot be connected to.
    pub blocked_subnets: HashSet<IpNet>,
}

impl ConnectionGater {
    /// The duration of a dial period.
    ///
    /// A peer cannot be dialed more than [`GossipDriverBuilder.peer_redialing`] times during a
    /// dial period. The dial period is reset once the last time the peer was dialed is longer
    /// than the dial period. This is to prevent peers from being dialed too often.
    ///
    /// TODO(@theochap): this should be configurable through CLI.
    const DIAL_PERIOD: Duration = Duration::from_secs(60 * 60);

    /// Creates a new instance of the `ConnectionGater`.
    pub fn new(peer_redialing: Option<u64>) -> Self {
        Self {
            peer_redialing,
            current_dials: HashSet::new(),
            dialed_peers: HashMap::new(),
            connectedness: HashMap::new(),
            protected_peers: HashSet::new(),
            blocked_peers: HashSet::new(),
            blocked_addrs: HashSet::new(),
            blocked_subnets: HashSet::new(),
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
        if dialed.num_dials >= redialing {
            return true;
        }
        false
    }

    fn dial_period_expired(&self, addr: &Multiaddr) -> bool {
        let Some(dial_info) = self.dialed_peers.get(addr) else {
            return false;
        };
        dial_info.last_dial.elapsed() > Self::DIAL_PERIOD
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

    /// Checks if a given [`IpAddr`] is within any of the `blocked_subnets`.
    pub fn check_ip_in_blocked_subnets(&self, ip_addr: &IpAddr) -> bool {
        for subnet in &self.blocked_subnets {
            if subnet.contains(ip_addr) {
                return true;
            }
        }
        false
    }
}

impl ConnectionGate for ConnectionGater {
    fn can_dial(&mut self, addr: &Multiaddr) -> bool {
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
        let protected = self.protected_peers.contains(&peer_id);

        // If the peer is not protected, its dial threshold is reached and dial period is not
        // expired, do not dial.
        if !protected && self.dial_threshold_reached(addr) && !self.dial_period_expired(addr) {
            debug!(target: "gossip", peer=?addr, "Dial threshold reached, not dialing");
            self.connectedness.insert(peer_id, Connectedness::CannotConnect);
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
            self.connectedness.insert(peer_id, Connectedness::CannotConnect);
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "blocked_address", "peer" => peer_id.to_string());
            return false;
        }

        // If address lies in any blocked subnets, do not dial.
        if self.check_ip_in_blocked_subnets(&ip_addr) {
            debug!(target: "gossip", ip=?ip_addr, "IP address is in a blocked subnet, not dialing");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "blocked_subnet", "peer" => peer_id.to_string());
            return false;
        }

        true
    }

    fn connectedness(&self, peer_id: &PeerId) -> Connectedness {
        self.connectedness.get(peer_id).cloned().unwrap_or(Connectedness::NotConnected)
    }

    fn list_protected_peers(&self) -> Vec<PeerId> {
        self.protected_peers.iter().copied().collect()
    }

    fn dialing(&mut self, addr: &Multiaddr) {
        if let Some(peer_id) = Self::peer_id_from_addr(addr) {
            self.current_dials.insert(peer_id);
            self.connectedness.insert(peer_id, Connectedness::Connected);
        } else {
            warn!(target: "p2p", peer=?addr, "Failed to extract PeerId from Multiaddr when dialing");
        }
    }

    fn dialed(&mut self, addr: &Multiaddr) {
        let dial_info = self
            .dialed_peers
            .entry(addr.clone())
            .or_insert(DialInfo { num_dials: 0, last_dial: Instant::now() });

        // If the last dial was longer than the dial period, reset the number of dials.
        if dial_info.last_dial.elapsed() > Self::DIAL_PERIOD {
            dial_info.num_dials = 0;
        }

        dial_info.num_dials += 1;
        dial_info.last_dial = Instant::now();
        trace!(target: "gossip", peer=?addr, "Dialed peer, current count: {}", dial_info.num_dials);
    }

    fn remove_dial(&mut self, peer_id: &PeerId) {
        self.current_dials.remove(peer_id);
    }

    fn can_disconnect(&self, addr: &Multiaddr) -> bool {
        let Some(peer_id) = Self::peer_id_from_addr(addr) else {
            warn!(target: "p2p", peer=?addr, "Failed to extract PeerId from Multiaddr when checking disconnect");
            // If we cannot extract the PeerId, disconnection is allowed.
            return true;
        };
        // If the peer is protected, do not disconnect.
        if !self.protected_peers.contains(&peer_id) {
            return true;
        }
        // Peer is protected, cannot disconnect.
        false
    }

    fn block_peer(&mut self, peer_id: &PeerId) {
        self.blocked_peers.insert(*peer_id);
        debug!(target: "gossip", peer=?peer_id, "Blocked peer");
        self.connectedness.insert(*peer_id, Connectedness::CannotConnect);
    }

    fn unblock_peer(&mut self, peer_id: &PeerId) {
        self.blocked_peers.remove(peer_id);
        debug!(target: "gossip", peer=?peer_id, "Unblocked peer");
        self.connectedness.insert(*peer_id, Connectedness::NotConnected);
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

    fn block_subnet(&mut self, subnet: IpNet) {
        self.blocked_subnets.insert(subnet);
        debug!(target: "gossip", ?subnet, "Blocked subnet");
    }

    fn unblock_subnet(&mut self, subnet: IpNet) {
        self.blocked_subnets.remove(&subnet);
        debug!(target: "gossip", ?subnet, "Unblocked subnet");
    }

    fn list_blocked_subnets(&self) -> Vec<IpNet> {
        self.blocked_subnets.iter().copied().collect()
    }

    fn protect_peer(&mut self, peer_id: PeerId) {
        self.protected_peers.insert(peer_id);
        debug!(target: "gossip", peer=?peer_id, "Protected peer");
    }

    fn unprotect_peer(&mut self, peer_id: PeerId) {
        self.protected_peers.remove(&peer_id);
        debug!(target: "gossip", peer=?peer_id, "Unprotected peer");
    }
}

#[test]
fn test_check_ip_in_blocked_subnets_ipv4() {
    use std::str::FromStr;

    let mut gater = ConnectionGater::new(None);
    gater.blocked_subnets.insert("192.168.1.0/24".parse::<IpNet>().unwrap());
    gater.blocked_subnets.insert("10.0.0.0/8".parse::<IpNet>().unwrap());
    gater.blocked_subnets.insert("172.16.0.0/16".parse::<IpNet>().unwrap());

    // IP in blocked subnet
    assert!(gater.check_ip_in_blocked_subnets(&IpAddr::from_str("192.168.1.100").unwrap()));
    assert!(gater.check_ip_in_blocked_subnets(&IpAddr::from_str("10.0.0.5").unwrap()));
    assert!(gater.check_ip_in_blocked_subnets(&IpAddr::from_str("172.16.255.255").unwrap()));

    // IP not in any blocked subnet
    assert!(!gater.check_ip_in_blocked_subnets(&IpAddr::from_str("192.168.2.1").unwrap()));
    assert!(!gater.check_ip_in_blocked_subnets(&IpAddr::from_str("172.17.0.1").unwrap()));
    assert!(!gater.check_ip_in_blocked_subnets(&IpAddr::from_str("8.8.8.8").unwrap()));
}
