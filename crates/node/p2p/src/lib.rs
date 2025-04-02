#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
extern crate tracing;

mod net;
pub use net::{Config, Network, NetworkBuilder, NetworkBuilderError};

mod rpc;
pub use rpc::{NetRpcRequest, NetworkRpc};

mod gossip;
pub use gossip::{
    Behaviour, BehaviourError, BlockHandler, DEFAULT_MESH_D, DEFAULT_MESH_DHI, DEFAULT_MESH_DLAZY,
    DEFAULT_MESH_DLO, Event, GLOBAL_VALIDATE_THROTTLE, GOSSIP_HEARTBEAT, GossipDriver,
    GossipDriverBuilder, GossipDriverBuilderError, Handler, HandlerEncodeError, MAX_GOSSIP_SIZE,
    MAX_OUTBOUND_QUEUE, MAX_VALIDATE_QUEUE, MIN_GOSSIP_SIZE, PEER_SCORE_INSPECT_FREQUENCY,
    PublishError, SEEN_MESSAGES_TTL, default_config, default_config_builder,
};

mod peers;
pub use peers::{
    AnyNode, BootNode, BootNodes, BootStore, NodeRecord, NodeRecordParseError, OP_RAW_BOOTNODES,
    OP_RAW_TESTNET_BOOTNODES, OpStackEnr, PeerId, enr_to_multiaddr,
};

mod discv5;
pub use discv5::{
    Discv5Builder, Discv5BuilderError, Discv5Driver, Discv5Handler, HandlerRequest, HandlerResponse,
};

mod utils;
pub use utils::{KeypairError, ParseKeyError, get_keypair, parse_key};
