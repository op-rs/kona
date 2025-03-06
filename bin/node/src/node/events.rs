//! Events for communicating between [NodeActor]s.
//!
//! [NodeActor]: crate::node::traits::NodeActor

use kona_protocol::{BlockInfo, L2BlockInfo};
use kona_rpc::OpAttributesWithParent;

/// Events emitted and processed by various [NodeActor]s in the system.
///
/// [NodeActor]: crate::node::traits::NodeActor
#[derive(Debug, Clone, derive_more::Display)]
#[allow(clippy::large_enum_variant)]
pub enum NodeEvent {
    /// An L1 head update event, with the corresponding [BlockInfo] of the new L1 head block.
    #[display("L1 head update: {}", _0)]
    L1HeadUpdate(BlockInfo),
    /// An L2 forkchoice update event.
    ///
    /// TODO: Extend to ForkchoiceState, mocked as just the safe head for now.
    #[display("L2 forkchoice update: {:?}", _0)]
    L2ForkchoiceUpdate(L2BlockInfo),
    /// A new payload was derived.
    #[display("Derived payload attributes: {:?}", _0)]
    DerivedPayload(OpAttributesWithParent),
}
