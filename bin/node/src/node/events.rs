//! Events for communicating between [NodeActor]s.
//!
//! [NodeActor]: crate::node::traits::NodeActor

use kona_protocol::BlockInfo;

/// Events emitted and processed by various [NodeActor]s in the system.
///
/// [NodeActor]: crate::node::traits::NodeActor
#[derive(Debug, Clone, derive_more::Display)]
pub enum NodeEvent {
    /// An L1 head update event, with the corresponding [BlockInfo] of the new L1 head block.
    #[display("L1 head update: {}", _0)]
    L1HeadUpdate(BlockInfo),
}
