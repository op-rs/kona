//! Standard implementation of the [RollupNode] service, using the governance approved
//! OP Stack configuration of components.
//!
//! See: <https://specs.optimism.io/protocol/rollup-node.html>

mod node;
pub use node::RollupNode;

mod error;
pub use error::RollupNodeError;

mod builder;
pub use builder::RollupNodeBuilder;
