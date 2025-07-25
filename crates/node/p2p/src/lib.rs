//! # kona-p2p
//!
//! A peer-to-peer networking library for the OP Stack, providing decentralized node communication
//! and coordination for Optimism rollups.
//!
//! ## Overview
//!
//! `kona-p2p` implements the networking layer for OP Stack nodes, enabling:
//! - **Peer Discovery**: Automatic discovery of network peers using Discv5
//! - **Block Gossip**: Efficient propagation of blocks and network payloads via GossipSub
//! - **Connection Management**: Intelligent peer connection gating and management
//! - **RPC Interface**: Administrative RPC API for network monitoring and control
//!
//! ## Architecture
//!
//! The library is organized into four main modules:
//!
//! - [`gossip`]: GossipSub-based block propagation and validation
//! - [`discv5`]: Peer discovery service using Ethereum's Discv5 protocol
//! - [`rpc`]: Administrative RPC API for network status and peer management
//! - [`metrics`]: Observability and monitoring capabilities
//!
//! ## Key Components
//!
//! ### GossipDriver
//! The [`GossipDriver`] manages the libp2p swarm and handles block gossip, implementing
//! the consensus-layer networking for Optimism. It validates and propagates
//! [`OpNetworkPayloadEnvelope`] messages across the network.
//!
//! ### Discv5Driver
//! The [`Discv5Driver`] provides peer discovery capabilities, maintaining a distributed
//! hash table of network peers and facilitating new peer connections.
//!
//! ### Connection Management
//! The library includes sophisticated connection gating through [`ConnectionGater`],
//! which implements rate limiting, IP filtering, and peer protection mechanisms.
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use kona_p2p::{GossipDriverBuilder, Discv5Builder};
//! use libp2p_identity::Keypair;
//! use std::net::Ipv4Addr;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a keypair for the node
//! let keypair = Keypair::generate_secp256k1();
//!
//! // Build the gossip driver
//! let gossip_driver = GossipDriverBuilder::new()
//!     .with_keypair(keypair.clone())
//!     .with_addr("127.0.0.1:9000".parse()?)
//!     .build()?;
//!
//! // Build the discovery service
//! let discv5_driver = Discv5Builder::new()
//!     .with_keypair(keypair)
//!     .with_listen_addr(Ipv4Addr::LOCALHOST, 9001)
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Features
//!
//! - `metrics`: Enable Prometheus metrics collection (optional)
//!
//! ## Technical Notes
//!
//! Unlike the reference `op-node` implementation, `kona-node` relies on the peer scoring
//! system built into `rust-libp2p` rather than implementing custom peer scoring. This
//! simplifies the implementation while maintaining network health through libp2p's
//! proven scoring mechanisms.
//!
//! ## Acknowledgements
//!
//! This implementation is largely based on [magi]'s [p2p module], adapted for the
//! `kona` ecosystem with additional features and OP Stack specific optimizations.
//!
//! [magi]: https://github.com/a16z/magi
//! [p2p module]: https://github.com/a16z/magi/tree/master/src/network

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
extern crate tracing;

/// Metrics collection and observability for the P2P stack.
///
/// Provides Prometheus-compatible metrics for monitoring network health,
/// peer connections, discovery events, and gossip activity.
mod metrics;
pub use metrics::Metrics;

/// RPC API types and request handling for P2P administration.
///
/// Provides a JSON-RPC compatible interface for monitoring and controlling
/// the P2P networking stack, including peer management, connection status,
/// and network statistics.
mod rpc;
pub use rpc::{
    Connectedness, Direction, GossipScores, P2pRpcRequest, PeerCount, PeerDump, PeerInfo,
    PeerScores, PeerStats, ReqRespScores, TopicScores,
};

/// GossipSub-based consensus layer networking for Optimism.
///
/// Implements block gossip, payload validation, and mesh networking using libp2p's
/// GossipSub protocol. Handles propagation of [`OpNetworkPayloadEnvelope`] messages
/// and maintains network health through connection gating and peer scoring.
mod gossip;
pub use gossip::{
    Behaviour, BehaviourError, BlockHandler, BlockInvalidError, ConnectionGate, ConnectionGater,
    DEFAULT_MESH_D, DEFAULT_MESH_DHI, DEFAULT_MESH_DLAZY, DEFAULT_MESH_DLO, DialError, DialInfo,
    Event, GLOBAL_VALIDATE_THROTTLE, GOSSIP_HEARTBEAT, GaterConfig, GossipDriver,
    GossipDriverBuilder, GossipDriverBuilderError, Handler, HandlerEncodeError, MAX_GOSSIP_SIZE,
    MAX_OUTBOUND_QUEUE, MAX_VALIDATE_QUEUE, MIN_GOSSIP_SIZE, PEER_SCORE_INSPECT_FREQUENCY,
    PublishError, SEEN_MESSAGES_TTL, default_config, default_config_builder,
};

/// Peer discovery service using Ethereum's Discv5 protocol.
///
/// Provides decentralized peer discovery through a distributed hash table (DHT),
/// enabling nodes to find and connect to other network participants. Handles
/// ENR (Ethereum Node Record) management and bootstrap node coordination.
mod discv5;
pub use discv5::{
    Discv5Builder, Discv5BuilderError, Discv5Driver, Discv5Handler, HandlerRequest, LocalNode,
};
