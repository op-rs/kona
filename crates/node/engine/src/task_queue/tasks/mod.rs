//! Tasks to update the engine state.

mod task;
pub use task::{
    EngineTask, EngineTaskError, EngineTaskErrorSeverity, EngineTaskErrors, EngineTaskExt,
};

mod synchronize;
pub use synchronize::{SynchronizeTask, SynchronizeTaskError};

mod insert;
pub use insert::{InsertTask, InsertTaskError};

mod build;
pub use build::{BuildTask, BuildTaskError, EngineBuildError};

mod consolidate;
pub use consolidate::{ConsolidateTask, ConsolidateTaskError};

mod finalize;
pub use finalize::{FinalizeTask, FinalizeTaskError};
