//! Network types

use alloc::{string::String, vec::Vec};
use core::net::IpAddr;

use alloy_primitives::{map::HashMap, ChainId};

/// Topic scores
///
/// <https://github.com/ethereum-optimism/optimism/blob/8dd17a7b114a7c25505cd2e15ce4e3d0f7e3f7c1/op-node/p2p/store/iface.go#L13>
#[derive(Clone, Debug, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct TopicScores {
    /// The time in the mesh.
    pub time_in_mesh: f64,
    /// First message deliveries
    pub first_message_deliveries: f64,
    /// Mesh message deliveries
    pub mesh_message_deliveries: f64,
    /// Invalid message deliveries
    pub invalid_message_deliveries: f64,
}

/// Gossip Scores
///
/// <https://github.com/ethereum-optimism/optimism/blob/8dd17a7b114a7c25505cd2e15ce4e3d0f7e3f7c1/op-node/p2p/store/iface.go#L20C6-L20C18>
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct GossipScores {
    /// The total number of gossip scores
    pub total: f64,
    /// Blocks involved
    pub blocks: TopicScores,
    /// The ip colocation factor.
    #[cfg_attr(feature = "serde", serde(rename = "IPColocationFactor"))]
    pub ip_colocation_factor: f64,
    /// The behavioral penalty.
    pub behavioral_penalty: f64,
}

/// The request response scores
///
/// <https://github.com/ethereum-optimism/optimism/blob/8dd17a7b114a7c25505cd2e15ce4e3d0f7e3f7c1/op-node/p2p/store/iface.go#L31C1-L35C2>
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ReqRespScores {
    /// Valid response count.
    pub valid_responses: f64,
    /// Error response count.
    pub error_responses: f64,
    /// Number of rejected payloads.
    pub rejected_payloads: f64,
}

/// Peer Scores
///
/// <https://github.com/ethereum-optimism/optimism/blob/8dd17a7b114a7c25505cd2e15ce4e3d0f7e3f7c1/op-node/p2p/store/iface.go#L81>
#[derive(Clone, Debug, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct PeerScores {
    /// The gossip scores
    pub gossip: GossipScores,
    /// The request-response scores.
    pub req_resp: ReqRespScores,
}

/// The peer info.
///
/// <https://github.com/ethereum-optimism/optimism/blob/develop/op-node/p2p/rpc_api.go#L15>
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct PeerInfo {
    /// The peer id.
    #[cfg_attr(feature = "serde", serde(rename = "peerID"))]
    pub peer_id: String,
    /// The node id.
    #[cfg_attr(feature = "serde", serde(rename = "nodeID"))]
    pub node_id: String,
    /// The user agent.
    pub user_agent: String,
    /// The protocol version.
    pub protocol_version: String,
    /// The enr for the peer.
    #[cfg_attr(feature = "serde", serde(rename = "ENR"))]
    pub enr: String,
    /// The peer addresses.
    pub addresses: Vec<String>,
    /// Peer supported protocols
    pub protocols: Option<Vec<String>>,
    /// 0: "`NotConnected`", 1: "Connected",
    /// 2: "`CanConnect`" (gracefully disconnected)
    /// 3: "`CannotConnect`" (tried but failed)
    pub connectedness: Connectedness,
    /// 0: "Unknown", 1: "Inbound" (if the peer contacted us)
    /// 2: "Outbound" (if we connected to them)
    pub direction: Direction,
    /// Whether the peer is protected.
    pub protected: bool,
    /// The chain id.
    #[cfg_attr(feature = "serde", serde(rename = "chainID"))]
    pub chain_id: ChainId,
    /// The peer latency in nanoseconds
    pub latency: u64,
    /// Whether the peer gossips
    pub gossip_blocks: bool,
    /// The peer scores.
    #[cfg_attr(feature = "serde", serde(rename = "scores"))]
    pub peer_scores: PeerScores,
}

/// A raw peer dump.
///
/// <https://github.com/ethereum-optimism/optimism/blob/40750a58e7a4a6f06370d18dfe6c6eab309012d9/op-node/p2p/rpc_api.go#L36>
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct PeerDump {
    /// The total number of connected peers
    pub total_connected: u32,
    /// A map from peer id to peer info
    pub peers: HashMap<String, PeerInfo>,
    /// A list of banned peers.
    pub banned_peers: Vec<String>,
    /// A list of banned ip addresses.
    #[cfg_attr(feature = "serde", serde(rename = "bannedIPS"))]
    pub banned_ips: Vec<IpAddr>,
    // TODO: should be IPNet
    /// The banned subnets
    pub banned_subnets: Vec<IpAddr>,
}

/// Peer stats.
///
/// <https://github.com/ethereum-optimism/optimism/blob/develop/op-node/p2p/rpc_server.go#L203>
#[derive(Clone, Debug, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct PeerStats {
    /// The number of connections
    pub connected: u32,
    /// The table.
    pub table: u32,
    /// The blocks topic.
    #[cfg_attr(feature = "serde", serde(rename = "blocksTopic"))]
    pub blocks_topic: u32,
    /// The blocks v2 topic.
    #[cfg_attr(feature = "serde", serde(rename = "blocksTopicV2"))]
    pub blocks_topic_v2: u32,
    /// The blocks v3 topic.
    #[cfg_attr(feature = "serde", serde(rename = "blocksTopicV3"))]
    pub blocks_topic_v3: u32,
    /// The banned count.
    pub banned: u32,
    /// The known count.
    pub known: u32,
}

/// Represents the connectivity state of a peer in a network, indicating the reachability and
/// interaction status of a node with its peers.
#[derive(Clone, Debug, PartialEq, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Connectedness {
    /// No current connection to the peer, and no recent history of a successful connection.
    #[default]
    NotConnected = 0,

    /// An active, open connection to the peer exists.
    Connected = 1,

    /// Connection to the peer is possible but not currently established; usually implies a past
    /// successful connection.
    CanConnect = 2,

    /// Recent attempts to connect to the peer failed, indicating potential issues in reachability
    /// or peer status.
    CannotConnect = 3,

    /// Connection to the peer is limited; may not have full capabilities.
    Limited = 4,
}

impl core::fmt::Display for Connectedness {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotConnected => write!(f, "Not Connected"),
            Self::Connected => write!(f, "Connected"),
            Self::CanConnect => write!(f, "Can Connect"),
            Self::CannotConnect => write!(f, "Cannot Connect"),
            Self::Limited => write!(f, "Limited"),
        }
    }
}

impl From<u8> for Connectedness {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::NotConnected,
            1 => Self::Connected,
            2 => Self::CanConnect,
            3 => Self::CannotConnect,
            4 => Self::Limited,
            _ => Self::NotConnected,
        }
    }
}
/// Direction represents the direction of a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Unknown is the default direction when the direction is not specified.
    #[default]
    Unknown = 0,
    /// Inbound is for when the remote peer initiated the connection.
    Inbound = 1,
    /// Outbound is for when the local peer initiated the connection.
    Outbound = 2,
}

#[cfg(feature = "serde")]
impl serde::Serialize for Direction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Direction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        match value {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::Inbound),
            2 => Ok(Self::Outbound),
            _ => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Unsigned(value as u64),
                &"a value between 0 and 2",
            )),
        }
    }
}

impl core::fmt::Display for Direction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Unknown => "Unknown",
                Self::Inbound => "Inbound",
                Self::Outbound => "Outbound",
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn test_connectedness_from_u8() {
        assert_eq!(Connectedness::from(0), Connectedness::NotConnected);
        assert_eq!(Connectedness::from(1), Connectedness::Connected);
        assert_eq!(Connectedness::from(2), Connectedness::CanConnect);
        assert_eq!(Connectedness::from(3), Connectedness::CannotConnect);
        assert_eq!(Connectedness::from(4), Connectedness::Limited);
        assert_eq!(Connectedness::from(5), Connectedness::NotConnected);
    }

    #[test]
    fn test_connectedness_display() {
        assert_eq!(Connectedness::NotConnected.to_string(), "Not Connected");
        assert_eq!(Connectedness::Connected.to_string(), "Connected");
        assert_eq!(Connectedness::CanConnect.to_string(), "Can Connect");
        assert_eq!(Connectedness::CannotConnect.to_string(), "Cannot Connect");
        assert_eq!(Connectedness::Limited.to_string(), "Limited");
    }

    #[test]
    fn test_direction_display() {
        assert_eq!(Direction::Unknown.to_string(), "Unknown");
        assert_eq!(Direction::Inbound.to_string(), "Inbound");
        assert_eq!(Direction::Outbound.to_string(), "Outbound");
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_direction_serialization() {
        assert_eq!(
            serde_json::to_string(&Direction::Unknown).unwrap(),
            "0",
            "Serialization failed for Direction::Unknown"
        );
        assert_eq!(
            serde_json::to_string(&Direction::Inbound).unwrap(),
            "1",
            "Serialization failed for Direction::Inbound"
        );
        assert_eq!(
            serde_json::to_string(&Direction::Outbound).unwrap(),
            "2",
            "Serialization failed for Direction::Outbound"
        );
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_direction_deserialization() {
        let unknown: Direction = serde_json::from_str("0").unwrap();
        let inbound: Direction = serde_json::from_str("1").unwrap();
        let outbound: Direction = serde_json::from_str("2").unwrap();

        assert_eq!(unknown, Direction::Unknown, "Deserialization mismatch for Direction::Unknown");
        assert_eq!(inbound, Direction::Inbound, "Deserialization mismatch for Direction::Inbound");
        assert_eq!(
            outbound,
            Direction::Outbound,
            "Deserialization mismatch for Direction::Outbound"
        );
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_peer_info_connectedness_serialization() {
        let peer_info = PeerInfo {
            peer_id: String::from("peer123"),
            node_id: String::from("node123"),
            user_agent: String::from("MyUserAgent"),
            protocol_version: String::from("v1"),
            enr: String::from("enr123"),
            addresses: [String::from("127.0.0.1")].to_vec(),
            protocols: Some([String::from("eth"), String::from("p2p")].to_vec()),
            connectedness: Connectedness::Connected,
            direction: Direction::Outbound,
            protected: true,
            chain_id: 1,
            latency: 100,
            gossip_blocks: true,
            peer_scores: PeerScores {
                gossip: GossipScores {
                    total: 1.0,
                    blocks: TopicScores {
                        time_in_mesh: 10.0,
                        first_message_deliveries: 5.0,
                        mesh_message_deliveries: 2.0,
                        invalid_message_deliveries: 0.0,
                    },
                    ip_colocation_factor: 0.5,
                    behavioral_penalty: 0.1,
                },
                req_resp: ReqRespScores {
                    valid_responses: 10.0,
                    error_responses: 1.0,
                    rejected_payloads: 0.0,
                },
            },
        };

        let serialized = serde_json::to_string(&peer_info).expect("Serialization failed");

        let deserialized: PeerInfo =
            serde_json::from_str(&serialized).expect("Deserialization failed");

        assert_eq!(peer_info.peer_id, deserialized.peer_id);
        assert_eq!(peer_info.node_id, deserialized.node_id);
        assert_eq!(peer_info.user_agent, deserialized.user_agent);
        assert_eq!(peer_info.protocol_version, deserialized.protocol_version);
        assert_eq!(peer_info.enr, deserialized.enr);
        assert_eq!(peer_info.addresses, deserialized.addresses);
        assert_eq!(peer_info.protocols, deserialized.protocols);
        assert_eq!(peer_info.connectedness, deserialized.connectedness);
        assert_eq!(peer_info.direction, deserialized.direction);
        assert_eq!(peer_info.protected, deserialized.protected);
        assert_eq!(peer_info.chain_id, deserialized.chain_id);
        assert_eq!(peer_info.latency, deserialized.latency);
        assert_eq!(peer_info.gossip_blocks, deserialized.gossip_blocks);
        assert_eq!(peer_info.peer_scores.gossip.total, deserialized.peer_scores.gossip.total);
        assert_eq!(
            peer_info.peer_scores.req_resp.valid_responses,
            deserialized.peer_scores.req_resp.valid_responses
        );
    }
}
