//! Contains the p2p RPC request type.

use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    num::TryFromIntError,
};

use crate::{Discv5Handler, GossipDriver};
use alloy_primitives::map::foldhash::fast::RandomState;
use discv5::{
    enr::{NodeId, k256::ecdsa},
    multiaddr::Protocol,
};
use kona_peers::OpStackEnr;
use libp2p::PeerId;
use tokio::sync::oneshot::Sender;

use libp2p::{Multiaddr, gossipsub::TopicHash};

use super::{
    PeerDump, PeerStats,
    types::{Connectedness, Direction, PeerInfo, PeerScores},
};
use crate::ConnectionGate;

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
    /// Request to block a peer by its [`PeerId`].
    BlockPeer {
        /// The [`PeerId`] of the peer to block.
        id: PeerId,
    },
    /// Request to unblock a peer by its [`PeerId`].
    UnblockPeer {
        /// The [`PeerId`] of the peer to unblock.
        id: PeerId,
    },
    /// Request to list all blocked peers.
    ListBlockedPeers(Sender<Vec<PeerId>>),
    /// Request to block a given IP Address.
    BlockAddr {
        /// The IP address to block.
        address: IpAddr,
    },
    /// Request to unblock a given IP Address.
    UnblockAddr {
        /// The IP address to unblock.
        address: IpAddr,
    },
    /// Request to list all blocked IP Addresses.
    ListBlockedAddrs(Sender<Vec<IpAddr>>),
    /// Request to connect to a given peer.
    ConnectPeer {
        /// The [`Multiaddr`] of the peer to connect to.
        address: Multiaddr,
    },
    /// Request to disconnect the specified peer.
    DisconnectPeer {
        /// The peer id to disconnect.
        peer_id: PeerId,
    },
    /// Protects a given peer from disconnection.
    ProtectPeer {
        /// The id of the peer.
        peer_id: PeerId,
    },
    /// Unprotects a given peer.
    UnprotectPeer {
        /// The id of the peer.
        peer_id: PeerId,
    },
    /// Returns the current peer stats for both the
    /// - Discovery Service ([`crate::Discv5Driver`])
    /// - Gossip Service ([`crate::GossipDriver`])
    ///
    /// This information can be used to briefly monitor the current state of the p2p network for a
    /// given peer.
    PeerStats(Sender<PeerStats>),
}

impl P2pRpcRequest {
    /// Handles the peer count request.
    pub fn handle<G: ConnectionGate>(self, gossip: &mut GossipDriver<G>, disc: &Discv5Handler) {
        match self {
            Self::PeerCount(s) => Self::handle_peer_count(s, gossip, disc),
            Self::DiscoveryTable(s) => Self::handle_discovery_table(s, disc),
            Self::PeerInfo(s) => Self::handle_peer_info(s, gossip, disc),
            Self::Peers { out, connected } => Self::handle_peers(out, connected, gossip, disc),
            Self::DisconnectPeer { peer_id } => Self::disconnect_peer(peer_id, gossip),
            Self::PeerStats(s) => Self::handle_peer_stats(s, gossip, disc),
            Self::ConnectPeer { address } => Self::connect_peer(address, gossip),
            Self::BlockPeer { id } => Self::block_peer(id, gossip),
            Self::UnblockPeer { id } => Self::unblock_peer(id, gossip),
            Self::ListBlockedPeers(s) => Self::list_blocked_peers(s, gossip),
            Self::BlockAddr { address } => Self::block_addr(address, gossip),
            Self::UnblockAddr { address } => Self::unblock_addr(address, gossip),
            Self::ListBlockedAddrs(s) => Self::list_blocked_addrs(s, gossip),
            Self::ProtectPeer { peer_id } => Self::protect_peer(peer_id, gossip),
            Self::UnprotectPeer { peer_id } => Self::unprotect_peer(peer_id, gossip),
        }
    }

    fn protect_peer<G: ConnectionGate>(id: PeerId, gossip: &mut GossipDriver<G>) {
        gossip.connection_gate.protect_peer(id);
    }

    fn unprotect_peer<G: ConnectionGate>(id: PeerId, gossip: &mut GossipDriver<G>) {
        gossip.connection_gate.unprotect_peer(id);
    }

    fn block_addr<G: ConnectionGate>(address: IpAddr, gossip: &mut GossipDriver<G>) {
        gossip.connection_gate.block_addr(address);
    }

    fn unblock_addr<G: ConnectionGate>(address: IpAddr, gossip: &mut GossipDriver<G>) {
        gossip.connection_gate.unblock_addr(address);
    }

    fn list_blocked_addrs<G: ConnectionGate>(s: Sender<Vec<IpAddr>>, gossip: &GossipDriver<G>) {
        let blocked_addrs = gossip.connection_gate.list_blocked_addrs();
        if let Err(e) = s.send(blocked_addrs) {
            warn!(target: "p2p::rpc", "Failed to send blocked addresses through response channel: {:?}", e);
        }
    }

    fn block_peer<G: ConnectionGate>(id: PeerId, gossip: &mut GossipDriver<G>) {
        gossip.connection_gate.block_peer(&id);
        gossip.swarm.behaviour_mut().gossipsub.blacklist_peer(&id);
    }

    fn unblock_peer<G: ConnectionGate>(id: PeerId, gossip: &mut GossipDriver<G>) {
        gossip.connection_gate.unblock_peer(&id);
        gossip.swarm.behaviour_mut().gossipsub.remove_blacklisted_peer(&id);
    }

    fn list_blocked_peers<G: ConnectionGate>(s: Sender<Vec<PeerId>>, gossip: &GossipDriver<G>) {
        let blocked_peers = gossip.connection_gate.list_blocked_peers();
        if let Err(e) = s.send(blocked_peers) {
            warn!(target: "p2p::rpc", "Failed to send blocked peers through response channel: {:?}", e);
        }
    }

    fn connect_peer<G: ConnectionGate>(address: Multiaddr, gossip: &mut GossipDriver<G>) {
        gossip.dial_multiaddr(address)
    }

    fn disconnect_peer<G: ConnectionGate>(peer_id: PeerId, gossip: &mut GossipDriver<G>) {
        if let Err(e) = gossip.swarm.disconnect_peer_id(peer_id) {
            warn!(target: "p2p::rpc", "Failed to disconnect peer {}: {:?}", peer_id, e);
        } else {
            info!(target: "p2p::rpc", "Disconnected peer {}", peer_id);
            // Record the duration of the peer connection.
            if let Some(start_time) = gossip.peer_connection_start.remove(&peer_id) {
                let peer_duration = start_time.elapsed();
                kona_macros::record!(
                    histogram,
                    crate::Metrics::GOSSIP_PEER_CONNECTION_DURATION_SECONDS,
                    peer_duration.as_secs_f64()
                );
            }
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

    fn handle_peers<G: ConnectionGate>(
        sender: Sender<PeerDump>,
        connected: bool,
        gossip: &GossipDriver<G>,
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

        // Get connection gate information.
        let banned_subnets = gossip.connection_gate.list_blocked_subnets();
        let banned_ips = gossip.connection_gate.list_blocked_addrs();
        let banned_peers = gossip.connection_gate.list_blocked_peers();
        let protected_peers = gossip.connection_gate.list_protected_peers();

        #[derive(Default)]
        struct PeerMetadata {
            protocols: Option<Vec<String>>,
            addresses: Vec<String>,
            user_agent: String,
            protocol_version: String,
        }

        // Build a map of peer ids to their supported protocols and addresses.
        let mut peer_metadata: HashMap<PeerId, PeerMetadata> = gossip
            .peer_infos
            .iter()
            .map(|(id, info)| {
                let protocols = if info.protocols.is_empty() {
                    None
                } else {
                    Some(
                        info.protocols
                            .iter()
                            .map(|protocol| protocol.to_string())
                            .collect::<Vec<String>>(),
                    )
                };
                let addresses =
                    info.listen_addrs.iter().map(|addr| addr.to_string()).collect::<Vec<String>>();
                (
                    *id,
                    PeerMetadata {
                        protocols,
                        addresses,
                        user_agent: info.agent_version.clone(),
                        protocol_version: info.protocol_version.clone(),
                    },
                )
            })
            .collect();

        // We consider that kona-nodes are gossiping blocks if their peers are subscribed to any of
        // the blocks topics.
        // This is the same heuristic as the one used in the op-node (`<https://github.com/ethereum-optimism/optimism/blob/6a8b2349c29c2a14f948fcb8aefb90526130acec/op-node/p2p/rpc_server.go#L179-L183>`).
        let peer_gossip_info = gossip
            .swarm
            .behaviour()
            .gossipsub
            .all_peers()
            .filter_map(|(peer_id, topics)| {
                let supported_topics = HashSet::from([
                    gossip.handler.blocks_v1_topic.hash(),
                    gossip.handler.blocks_v2_topic.hash(),
                    gossip.handler.blocks_v3_topic.hash(),
                    gossip.handler.blocks_v4_topic.hash(),
                ]);

                if topics.iter().any(|topic| supported_topics.contains(topic)) {
                    Some(*peer_id)
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();

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
                    let opstack_enr = OpStackEnr::try_from(enr).ok();

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
                        let PeerMetadata { protocols, addresses, user_agent, protocol_version } =
                            peer_metadata.remove(peer_id).unwrap_or_default();

                        let node_id = format!("{:?}", &enr.node_id());
                        (
                            peer_id.to_string(),
                            PeerInfo {
                                peer_id: peer_id.to_string(),
                                node_id,
                                user_agent,
                                protocol_version,
                                enr: enr.to_string(),
                                addresses,
                                protocols,
                                connectedness,
                                direction,
                                // Note: we use the chain id from the ENR if it exists, otherwise we
                                // use 0 to be consistent with op-node's behavior (`<https://github.com/ethereum-optimism/optimism/blob/6a8b2349c29c2a14f948fcb8aefb90526130acec/op-service/apis/p2p.go#L55>`).
                                chain_id: opstack_enr.map(|enr| enr.chain_id).unwrap_or(0),
                                gossip_blocks: peer_gossip_info.contains(peer_id),
                                protected: protected_peers.contains(peer_id),
                                // TODO(@theochap, `<https://github.com/op-rs/kona/issues/1562>`): support these fields
                                latency: 0,
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
                banned_peers: banned_peers.into_iter().map(|p| p.to_string()).collect(),
                banned_ips,
                banned_subnets,
            }) {
                warn!(target: "p2p::rpc", "Failed to send peer info through response channel: {:?}", e);
            }
        });
    }

    /// Handles a peer info request by spawning a task.
    fn handle_peer_info<G: ConnectionGate>(
        sender: Sender<PeerInfo>,
        gossip: &GossipDriver<G>,
        disc: &Discv5Handler,
    ) {
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
                    warn!(target: "p2p::rpc", "Failed to receive local ENR: {:?}", e);
                    return;
                }
            };

            // Note: we need to use `Debug` impl here because the `Display` impl of
            // `NodeId` strips some part of the hex string and replaces it with "...".
            let node_id = format!("{:?}", &enr.node_id());

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

    fn handle_peer_stats<G: ConnectionGate>(
        sender: Sender<PeerStats>,
        gossip: &GossipDriver<G>,
        disc: &Discv5Handler,
    ) {
        let peers_known = gossip.peerstore.len();
        let gossip_network_info = gossip.swarm.network_info();
        let table_info = disc.peer_count();

        let banned_peers = gossip.connection_gate.list_blocked_peers().len();

        let topics = gossip.swarm.behaviour().gossipsub.topics().collect::<HashSet<_>>();

        let topics = topics
            .into_iter()
            .map(|hash| {
                (
                    hash.clone(),
                    gossip
                        .swarm
                        .behaviour()
                        .gossipsub
                        .all_peers()
                        .filter(|(_, topics)| topics.contains(&hash))
                        .count(),
                )
            })
            .collect::<HashMap<_, _>>();

        let v1_topic_hash = gossip.handler.blocks_v1_topic.hash();
        let v2_topic_hash = gossip.handler.blocks_v2_topic.hash();
        let v3_topic_hash = gossip.handler.blocks_v3_topic.hash();
        let v4_topic_hash = gossip.handler.blocks_v4_topic.hash();

        tokio::spawn(async move {
            let Ok(table) = table_info.await else {
                error!(target: "p2p::rpc", "failed to get discovery table size. The sender has been dropped. The discv5 service may not be running anymore.");
                return;
            };

            let Ok(table) = table.try_into() else {
                error!(target: "p2p::rpc", "failed to get discovery table size. Integer overflow. Please ensure that the number of peers in the discovery table fits in a u32.");
                return;
            };

            let Ok(connected) = gossip_network_info.num_peers().try_into() else {
                error!(target: "p2p::rpc", "failed to get number of connected peers. Integer overflow. Please ensure that the number of connected peers fits in a u32.");
                return;
            };

            let Ok(known) = peers_known.try_into() else {
                error!(target: "p2p::rpc", "failed to get number of known peers. Integer overflow. Please ensure that the number of known peers fits in a u32.");
                return;
            };

            // Given a topic hash, this method:
            // - gets the number of peers in the mesh for that topic
            // - returns an error if the number of peers in the mesh overflows a u32
            // - returns 0 if there are no peers in the mesh for that topic
            let get_topic = |topic: &TopicHash| {
                Ok::<u32, TryFromIntError>(
                    topics
                        .get(topic)
                        .cloned()
                        .map(|v| v.try_into())
                        .transpose()?
                        .unwrap_or_default(),
                )
            };

            let Ok(block_topics) = vec![
                get_topic(&v1_topic_hash),
                get_topic(&v2_topic_hash),
                get_topic(&v3_topic_hash),
                get_topic(&v4_topic_hash),
            ]
            .into_iter()
            .collect::<Result<Vec<_>, _>>() else {
                error!(target: "p2p::rpc", "failed to get blocks topic. Some topic count overflowed. Make sure that the number of peers for a given topic fits in a u32.");
                return;
            };

            let stats = PeerStats {
                connected,
                table,
                blocks_topic: block_topics[0],
                blocks_topic_v2: block_topics[1],
                blocks_topic_v3: block_topics[2],
                blocks_topic_v4: block_topics[3],
                banned: banned_peers as u32,
                known,
            };

            if let Err(e) = sender.send(stats) {
                warn!("Failed to send peer stats through response channel: {:?}", e);
            };
        });
    }

    /// Handles a peer count request by spawning a task.
    fn handle_peer_count<G: ConnectionGate>(
        sender: Sender<(Option<usize>, usize)>,
        gossip: &GossipDriver<G>,
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
