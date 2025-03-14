//! Task to insert an unsafe payload into the execution engine.

mod task;
pub use task::{InsertTask, InsertTaskExt};

mod error;
pub use error::InsertTaskError;

mod messages;
pub use messages::{InsertTaskInput, InsertTaskOut};
