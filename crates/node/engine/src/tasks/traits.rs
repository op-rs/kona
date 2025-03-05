//! A trait that abstracts an engine api task.

use async_trait::async_trait;

/// An Engine Task.
#[async_trait]
pub trait EngineTask {
    /// An error type.
    type Error;

    /// Executes the task.
    async fn execute(&mut self) -> Result<(), Self::Error>;
}
