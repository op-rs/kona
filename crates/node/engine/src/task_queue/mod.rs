//! The [Engine] task queue and the [EngineTask]s it can execute.

mod core;
pub use core::{ConsolidationError, Engine};

mod tasks;
pub use tasks::*;
