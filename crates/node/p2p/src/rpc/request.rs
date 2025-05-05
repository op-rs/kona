//! Contains the p2p RPC request type.

use crate::{Discv5Handler, GossipDriver};
use discv5::multiaddr::Protocol;
use tokio::sync::oneshot::Sender;

use super::types::{Connectedness, Direction, PeerInfo, PeerScores};

/// A p2p RPC Request.
#[derive(Debug)]
pub enum P2pRpcRequest {
    /// Returns [`PeerInfo`] for the [`crate::Network`].
    PeerInfo(Sender<PeerInfo>),
    /// Dumps the node's discovery table from the [`crate::Discv5Driver`].
    DiscoveryTable(Sender<Vec<String>>),
    /// Returns the current peer count for both the
    /// - Discovery Service ([`crate::Discv5Driver`])
    /// - Gossip Service ([`crate::GossipDriver`])
    PeerCount(Sender<(Option<usize>, usize)>),
}

impl P2pRpcRequest {
    /// Handles the peer count request.
    pub fn handle(self, gossip: &GossipDriver, disc: &Discv5Handler) {
        match self {
            Self::PeerCount(s) => Self::handle_peer_count(s, gossip, disc),
            Self::DiscoveryTable(s) => Self::handle_discovery_table(s, disc),
            Self::PeerInfo(s) => Self::handle_peer_info(s, gossip, disc),
        }
    }

    fn handle_discovery_table(sender: Sender<Vec<String>>, disc: &Discv5Handler) {
        let enrs = disc.table_enrs();
        tokio::spawn(async move {
            let dt = match enrs.await {
                Ok(dt) => dt.into_iter().map(|e| e.to_string()).collect(),

                Err(e) => {
                    warn!("Failed to receive peer count: {:?}", e);
                    return;
                }
            };

            if let Err(e) = sender.send(dt) {
                warn!("Failed to send peer count through response channel: {:?}", e);
            }
        });
    }

    /// Handles a peer info request by spawning a task.
    fn handle_peer_info(sender: Sender<PeerInfo>, gossip: &GossipDriver, disc: &Discv5Handler) {
        let peer_id = *gossip.local_peer_id();
        let chain_id = disc.chain_id;
        let local_enr = disc.local_enr();
        let addresses = gossip
            .swarm
            .listeners()
            .map(|a| {
                let mut addr = a.clone();
                addr.push(Protocol::P2p(peer_id));
                addr.to_string()
            })
            .collect::<Vec<String>>();
        tokio::spawn(async move {
            let enr = match local_enr.await {
                Ok(enr) => enr,
                Err(e) => {
                    warn!("Failed to receive local ENR: {:?}", e);
                    return;
                }
            };
            let node_id = enr.node_id().to_string();

            // We need to add the local multiaddr to the list of known addresses.
            let peer_info = PeerInfo {
                peer_id: peer_id.to_string(),
                node_id,
                user_agent: "kona".to_string(),
                protocol_version: "1".to_string(),
                enr: enr.to_string(),
                addresses,
                protocols: None,
                connectedness: Connectedness::Connected,
                direction: Direction::Inbound,
                protected: false,
                chain_id,
                latency: 0,
                gossip_blocks: true,
                peer_scores: PeerScores::default(),
            };
            if let Err(e) = sender.send(peer_info) {
                warn!("Failed to send peer info through response channel: {:?}", e);
            }
        });
    }

    /// Handles a peer count request by spawning a task.
    fn handle_peer_count(
        sender: Sender<(Option<usize>, usize)>,
        gossip: &GossipDriver,
        disc: &Discv5Handler,
    ) {
        let pc_req = disc.peer_count();
        let gossip_pc = gossip.connected_peers();
        tokio::spawn(async move {
            let pc = match pc_req.await {
                Ok(pc) => Some(pc),
                Err(e) => {
                    warn!("Failed to receive peer count: {:?}", e);
                    None
                }
            };
            if let Err(e) = sender.send((pc, gossip_pc)) {
                warn!("Failed to send peer count through response channel: {:?}", e);
            }
        });
    }
}
