//! A trait that abstracts an engine api task.

use async_trait::async_trait;

/// An Engine Task.
#[async_trait]
pub trait EngineTask {
    /// The error returned by the task if execution fails.
    type Error;
    /// An input type to the execution method.
    type Input;

    /// Executes the task.
    async fn execute(&mut self, input: Self::Input) -> Result<(), Self::Error>;
}
