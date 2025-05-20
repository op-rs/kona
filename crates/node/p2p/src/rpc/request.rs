//! Contains the p2p RPC request type.

use std::collections::HashMap;

use crate::{Discv5Handler, GossipDriver};
use alloy_primitives::map::foldhash::fast::RandomState;
use discv5::{
    enr::{NodeId, k256::ecdsa},
    multiaddr::Protocol,
};
use libp2p::PeerId;
use tokio::sync::oneshot::Sender;

use super::{
    PeerDump,
    types::{Connectedness, Direction, PeerInfo, PeerScores},
};

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
    /// Returns a [`PeerDump`] containing detailed information about connected peers.
    /// If `connected` is true, only returns connected peers.
    Peers {
        /// The output channel to send the [`PeerDump`] to.
        out: Sender<PeerDump>,
        /// Whether to only return connected peers.
        connected: bool,
    },
}

impl P2pRpcRequest {
    /// Handles the peer count request.
    pub fn handle(self, gossip: &GossipDriver, disc: &Discv5Handler) {
        match self {
            Self::PeerCount(s) => Self::handle_peer_count(s, gossip, disc),
            Self::DiscoveryTable(s) => Self::handle_discovery_table(s, disc),
            Self::PeerInfo(s) => Self::handle_peer_info(s, gossip, disc),
            Self::Peers { out, connected } => Self::handle_peers(out, connected, gossip, disc),
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

    fn handle_peers(
        sender: Sender<PeerDump>,
        connected: bool,
        gossip: &GossipDriver,
        disc: &Discv5Handler,
    ) {
        let Ok(total_connected) = gossip.swarm.network_info().num_peers().try_into() else {
            error!(target: "p2p::rpc", "Failed to get total connected peers. The number of connected peers is too large and overflows u32.");
            return;
        };

        let peer_ids: Vec<PeerId> = if connected {
            gossip.swarm.connected_peers().cloned().collect()
        } else {
            gossip.peerstore.keys().cloned().collect()
        };

        // Build a map of peer ids to their supported protocols.
        let mut protocols: HashMap<PeerId, Vec<String>> =
            gossip.swarm.behaviour().gossipsub.peer_protocol().fold(
                HashMap::new(),
                |mut protocols, (id, protocol)| {
                    protocols.entry(*id).or_default().push(protocol.to_string());
                    protocols
                },
            );

        // Build a map of peer ids to their known addresses.
        let mut addresses: HashMap<PeerId, Vec<String>> =
        peer_ids.iter().filter_map(|id| {
            gossip.peerstore.get(id).or_else(|| {
                warn!(target: "p2p::rpc", "Failed to get peer address. The peerstore is not in sync with the gossip swarm.");
                None
            }).map(|addr| (*id, vec![addr.to_string()]))
        }).collect();

        let disc_table_infos = disc.table_infos();

        tokio::spawn(async move {
            let Ok(table_infos) = disc_table_infos.await else {
                error!(target: "p2p::rpc", "Failed to get table infos. The connection to the gossip driver is closed.");
                return;
            };

            let node_to_peer_id: HashMap<NodeId, PeerId> = peer_ids.into_iter().filter_map(|id|
            {
                let Ok(pubkey) = libp2p_identity::PublicKey::try_decode_protobuf(&id.to_bytes()[2..]) else {
                    error!(target: "p2p::rpc", peer_id = ?id, "Failed to decode public key from peer id. This is a bug as all the peer ids should be decodable (because they come from secp256k1 public keys).");
                    return None;
                };

                let key =
                match pubkey.try_into_secp256k1().map_err(|err| err.to_string()).and_then(
                    |key| ecdsa::VerifyingKey::from_sec1_bytes(key.to_bytes().as_slice()).map_err(|err| err.to_string())
                )    {                     Ok(key) => key,
                        Err(err) => {
                            error!(target: "p2p::rpc", peer_id = ?id, err = ?err, "Failed to convert public key to secp256k1 public key. This is a bug.");
                            return None;
                        }};
                let node_id = NodeId::from(key);
                Some((node_id, id))
            }
            ).collect();

            let infos: HashMap<String, PeerInfo, RandomState> = table_infos
                .iter()
                .filter_map(|(id, enr, status)| {
                    // TODO(@theochap, `<https://github.com/op-rs/kona/issues/1562>`): improve the connectedness information to include the other
                    // variants.
                    let connectedness = if status.is_connected() {
                        Connectedness::Connected
                    } else {
                        Connectedness::NotConnected
                    };

                    let direction =
                        if status.is_incoming() { Direction::Inbound } else { Direction::Outbound };

                    node_to_peer_id.get(id).map(|peer_id| {
                        (
                            peer_id.to_string(),
                            PeerInfo {
                                peer_id: peer_id.to_string(),
                                node_id: enr.node_id().to_string(),
                                // TODO(@theochap, `<https://github.com/op-rs/kona/issues/1562>`): support these fields
                                user_agent: String::new(),
                                // TODO(@theochap): support these fields
                                protocol_version: String::new(),
                                enr: enr.to_string(),
                                addresses: addresses.remove(peer_id).unwrap_or_default(),
                                protocols: protocols.remove(peer_id),
                                connectedness,
                                direction,
                                // TODO(@theochap, `<https://github.com/op-rs/kona/issues/1562>`): support these fields
                                protected: false,
                                // TODO(@theochap, `<https://github.com/op-rs/kona/issues/1562>`): support these fields
                                chain_id: 0,
                                // TODO(@theochap, `<https://github.com/op-rs/kona/issues/1562>`): support these fields
                                latency: 0,
                                // TODO(@theochap, `<https://github.com/op-rs/kona/issues/1562>`): support these fields
                                gossip_blocks: false,
                                // TODO(@theochap, `<https://github.com/op-rs/kona/issues/1562>`): support these fields
                                peer_scores: PeerScores::default(),
                            },
                        )
                    })
                })
                .collect();

            if let Err(e) = sender.send(PeerDump {
                total_connected,
                peers: infos,

                // TODO(@theochap): support these fields
                banned_peers: vec![],
                banned_ips: vec![],
                banned_subnets: vec![],
            }) {
                warn!(target: "p2p::rpc", "Failed to send peer info through response channel: {:?}", e);
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
