//! An implementation of the [`ConnectionGate`] trait.

use crate::{Connectedness, ConnectionGate, DialError};
use ipnet::IpNet;
use libp2p::{Multiaddr, PeerId};
use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    time::Duration,
};
use tokio::time::Instant;

/// Dial information tracking for peer connection management.
///
/// Tracks connection attempt statistics for rate limiting and connection gating.
/// Used to prevent excessive connection attempts to the same peer within a
/// configured time window.
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

/// Configuration parameters for the connection gater.
///
/// Controls rate limiting, connection management, and peer protection policies
/// to maintain network health and prevent abuse.
#[derive(Debug, Clone)]
pub struct GaterConfig {
    /// Maximum number of connection attempts per dial period for a single peer.
    ///
    /// If set to `None`, unlimited redials are allowed. When set, prevents
    /// excessive connection attempts to unresponsive or problematic peers.
    pub peer_redialing: Option<u64>,

    /// Duration of the rate limiting window for peer connections.
    ///
    /// A peer cannot be dialed more than `peer_redialing` times during this
    /// period. The period resets after this duration has elapsed since the
    /// last dial attempt. Default is 1 hour.
    pub dial_period: Duration,
}

impl Default for GaterConfig {
    fn default() -> Self {
        Self { peer_redialing: None, dial_period: Duration::from_secs(60 * 60) }
    }
}

/// Connection Gater
///
/// A connection gate that regulates peer connections for the libp2p gossip swarm.
///
/// An implementation of the [`ConnectionGate`] trait.
#[derive(Default, Debug, Clone)]
pub struct ConnectionGater {
    /// The configuration for the connection gater.
    config: GaterConfig,
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
    /// Creates a new instance of the `ConnectionGater`.
    pub fn new(config: GaterConfig) -> Self {
        Self {
            config,
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
        let Some(redialing) = self.config.peer_redialing else {
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
        dial_info.last_dial.elapsed() > self.config.dial_period
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
    fn can_dial(&mut self, addr: &Multiaddr) -> Result<(), DialError> {
        // Get the peer id from the given multiaddr.
        let peer_id = Self::peer_id_from_addr(addr).ok_or_else(|| {
            warn!(target: "p2p", peer=?addr, "Failed to extract PeerId from Multiaddr");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "invalid_multiaddr");
            DialError::InvalidMultiaddr { addr: addr.clone() }
        })?;

        // Cannot dial a peer that is already being dialed.
        if self.current_dials.contains(&peer_id) {
            debug!(target: "gossip", peer=?addr, "Already dialing peer, not dialing");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "already_dialing", "peer" => peer_id.to_string());
            return Err(DialError::AlreadyDialing { peer_id });
        }

        // If the peer is protected, do not apply thresholds.
        let protected = self.protected_peers.contains(&peer_id);

        // If the peer is not protected, its dial threshold is reached and dial period is not
        // expired, do not dial.
        if !protected && self.dial_threshold_reached(addr) && !self.dial_period_expired(addr) {
            debug!(target: "gossip", peer=?addr, "Dial threshold reached, not dialing");
            self.connectedness.insert(peer_id, Connectedness::CannotConnect);
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "threshold_reached", "peer" => peer_id.to_string());
            return Err(DialError::ThresholdReached { addr: addr.clone() });
        }

        // If the peer is blocked, do not dial.
        if self.blocked_peers.contains(&peer_id) {
            debug!(target: "gossip", peer=?addr, "Peer is blocked, not dialing");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "blocked_peer", "peer" => peer_id.to_string());
            return Err(DialError::PeerBlocked { peer_id });
        }

        // There must be a reachable IP Address in the Multiaddr protocol stack.
        let ip_addr = Self::ip_from_addr(addr).ok_or_else(|| {
            warn!(target: "p2p", peer=?addr, "Failed to extract IpAddr from Multiaddr");
            DialError::InvalidIpAddress { addr: addr.clone() }
        })?;

        // If the address is blocked, do not dial.
        if self.blocked_addrs.contains(&ip_addr) {
            debug!(target: "gossip", peer=?addr, "Address is blocked, not dialing");
            self.connectedness.insert(peer_id, Connectedness::CannotConnect);
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "blocked_address", "peer" => peer_id.to_string());
            return Err(DialError::AddressBlocked { ip: ip_addr });
        }

        // If address lies in any blocked subnets, do not dial.
        if self.check_ip_in_blocked_subnets(&ip_addr) {
            debug!(target: "gossip", ip=?ip_addr, "IP address is in a blocked subnet, not dialing");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "blocked_subnet", "peer" => peer_id.to_string());
            return Err(DialError::SubnetBlocked { ip: ip_addr });
        }

        Ok(())
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
        if dial_info.last_dial.elapsed() > self.config.dial_period {
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

    let mut gater = ConnectionGater::new(GaterConfig {
        peer_redialing: None,
        dial_period: Duration::from_secs(60 * 60),
    });
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

#[test]
fn test_dial_error_handling() {
    use crate::{ConnectionGate, DialError};
    use std::str::FromStr;

    let mut gater = ConnectionGater::new(GaterConfig::default());

    // Test invalid multiaddr (missing peer ID)
    let invalid_addr = Multiaddr::from_str("/ip4/127.0.0.1/tcp/8080").unwrap();
    let result = gater.can_dial(&invalid_addr);
    assert!(matches!(result, Err(DialError::InvalidMultiaddr { .. })));

    // Test with valid address
    let valid_addr = Multiaddr::from_str(
        "/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp",
    )
    .unwrap();

    // First dial should succeed
    assert!(gater.can_dial(&valid_addr).is_ok());

    // Mark as dialing
    gater.dialing(&valid_addr);

    // Second dial should fail with AlreadyDialing
    let result = gater.can_dial(&valid_addr);
    assert!(matches!(result, Err(DialError::AlreadyDialing { .. })));
}
