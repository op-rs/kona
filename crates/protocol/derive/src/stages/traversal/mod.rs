//! Contains traversal stages of the derivation pipeline.

pub(crate) mod polling;
pub use polling::L1Traversal;

mod managed;
pub use managed::ManagedTraversal;
