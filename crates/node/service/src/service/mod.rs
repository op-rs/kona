//! Core [`RollupNode`] service, composing the available [`NodeActor`]s into various modes of
//! operation.
//!
//! [`NodeActor`]: crate::NodeActor

mod builder;
pub use builder::RollupNodeBuilder;

mod mode;
pub use mode::{InteropMode, NodeMode};

mod node;
pub use node::RollupNode;

pub(crate) mod util;
pub(crate) use util::spawn_and_wait;
