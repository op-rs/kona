//! Tasks to update the engine state.

mod traits;
pub use traits::EngineTask;

mod forkchoice;
pub use forkchoice::{ForkchoiceTask, ForkchoiceTaskError, ForkchoiceTaskInput, ForkchoiceTaskOut};
