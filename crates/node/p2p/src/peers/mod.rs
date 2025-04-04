//! Networking Utilities ported from reth.
//!
//! Much of this module is ported from
//! <https://github.com/paradigmxyz/reth/blob/0e087ae1c35502f0b8d128c64e4c57269af20c0e/crates/net/peers/src/lib.rs>.
//!
//! This module manages and converts Ethereum network entities such as node records, peer IDs, and
//! Ethereum Node Records (ENRs)
//!
//! ## Node Record Overview
//!
//! Ethereum uses different types of "node records" to represent peers on the network.
//!
//! The simplest way to identify a peer is by public key. This is the [`PeerId`] type, which usually
//! represents a peer's secp256k1 public key.
//!
//! A more complete representation of a peer is the [`NodeRecord`] type, which includes the peer's
//! IP address, the ports where it is reachable (TCP and UDP), and the peer's public key. This is
//! what is returned from discovery v4 queries.
//!
//! The most comprehensive node record type is the Ethereum Node Record ([`discv5::Enr`]), which is
//! a signed, versioned record that includes the information from a [`NodeRecord`] along with
//! additional metadata. This is the data structure returned from discovery v5 queries.
//!
//! When we need to deserialize an identifier that could be any of these three types ([`PeerId`],
//! [`NodeRecord`], and [`discv5::Enr`]), we use the [`AnyNode`] type, which is an enum over the
//! three types. [`AnyNode`] is used in reth's `admin_addTrustedPeer` RPC method.
//!
//! In short, the types are as follows:
//! - [`PeerId`]: A simple public key identifier.
//! - [`NodeRecord`]: A more complete representation of a peer, including IP address and ports.
//! - [`discv5::Enr`]: An Ethereum Node Record, which is a signed, versioned record that includes
//!   additional metadata. Useful when interacting with discovery v5, or when custom metadata is
//!   required.
//! - [`AnyNode`]: An enum over [`PeerId`], [`NodeRecord`], and [`discv5::Enr`], useful in
//!   deserialization when the type of the node record is not known.

/// Alias for a peer identifier.
///
/// This is the most primitive secp256k1 public key identifier for a given peer.
pub type PeerId = alloy_primitives::B512;

mod nodes;
pub use nodes::{BootNodes, OP_RAW_BOOTNODES, OP_RAW_TESTNET_BOOTNODES};

mod store;
pub use store::BootStore;

mod score;
pub use score::PeerScoreLevel;

mod enr;
pub use enr::OpStackEnr;

mod any;
pub use any::AnyNode;

mod boot;
pub use boot::BootNode;

mod record;
pub use record::{NodeRecord, NodeRecordParseError};

mod utils;
pub use utils::enr_to_multiaddr;
