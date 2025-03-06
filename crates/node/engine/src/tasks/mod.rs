//! Tasks to update the engine state.

mod insert;
pub use insert::{InsertTask, InsertTaskError, InsertTaskExt, InsertTaskInput, InsertTaskOut};

mod traits;
pub use traits::EngineTask;

mod forkchoice;
pub use forkchoice::{
    ForkchoiceTask, ForkchoiceTaskError, ForkchoiceTaskExt, ForkchoiceTaskInput, ForkchoiceTaskOut,
};
