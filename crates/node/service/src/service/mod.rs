//! Core [RollupNode] service, composing the available [NodeActor]s into various modes of operation.
//!
//! [NodeActor]: crate::NodeActor

mod core;
pub use core::{NodeMode, RollupNodeService};

mod validator;
pub use validator::ValidatorNodeService;

mod sequencer;
pub use sequencer::SequencerNodeService;

mod standard;
pub use standard::{RollupNode, RollupNodeBuilder, RollupNodeError};

pub(crate) mod util;
pub(crate) use util::spawn_and_wait;
