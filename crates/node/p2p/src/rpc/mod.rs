//! Contains RPC types specific to Kona's p2p stack.

mod request;
pub use request::P2pRpcRequest;

mod types;
pub use types::{
    Connectedness, Direction, GossipScores, PeerCount, PeerDump, PeerInfo, PeerScores, PeerStats,
    ReqRespScores, TopicScores,
};
