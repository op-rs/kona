//! Contains various traversal stages for kona's derivation pipeline.
//!
//! The traversal stage sits at the bottom of the pipeline, and is responsible for
//! providing the next block to the next stage in the pipeline.
//!
//! ## Types
//!
//! - [`ManagedTraversal`]: A passive traversal stage that receives the next block through a signal.
//! - [`PollingTraversal`]: An active traversal stage that polls for the next block through its
//!   provider.

mod managed;
pub use managed::ManagedTraversal;

mod polling;
pub use polling::PollingTraversal;
