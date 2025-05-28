//! Tasks to update the engine state.

mod task;
pub use task::{EngineTask, EngineTaskError, EngineTaskExt};

mod forkchoice;
pub use forkchoice::{ForkchoiceTask, ForkchoiceTaskError};

mod insert;
pub use insert::{InsertUnsafeTask, InsertUnsafeTaskError};

mod build;
pub use build::{BuildTask, BuildTaskError};

mod consolidate;
pub use consolidate::{ConsolidateTask, ConsolidateTaskError};

mod finalize;
pub use finalize::{FinalizeTask, FinalizeTaskError};
