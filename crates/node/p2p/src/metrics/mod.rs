//! Metrics for the P2P stack.

/// Identifier for the gauge that tracks gossip events.
pub const GOSSIP_EVENT: &str = "Kona_node_gossip_events";

/// Identifier for the gauge that tracks unsafe blocks published.
pub const UNSAFE_BLOCK_PUBLISHED: &str = "Kona_node_unsafe_block_published";

/// Identifier for the gauge that tracks the number of connected peers.
pub const PEER_COUNT: &str = "Kona_node_peer_count";

/// Identifier for the gauge that tracks the number of dialed peers.
pub const DIAL_PEER: &str = "Kona_node_dial_peer";
