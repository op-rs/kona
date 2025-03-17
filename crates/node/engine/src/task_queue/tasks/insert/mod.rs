//! Task to insert an unsafe payload into the execution engine.

mod task;
pub use task::InsertUnsafeTask;

mod error;
pub use error::InsertUnsafeTaskError;
