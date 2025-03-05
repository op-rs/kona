//! A task to insert an unsafe payload into the execution engine.

use crate::{EngineTask, InsertTaskError, InsertTaskInput, InsertTaskOut};

/// A strongly typed receiver channel.
type Receiver = tokio::sync::broadcast::Receiver<InsertTaskInput>;

/// A strongly typed sender channel.
type Sender = tokio::sync::broadcast::Sender<InsertTaskOut>;

/// The task to insert an unsafe payload into the execution engine.
#[derive(Debug)]
pub struct InsertTask {
    /// A receiver channel to receive messages from an external actor.
    pub receiver: Receiver,
    /// A sender channel to send messages out to an external actor.
    pub sender: Sender,
}

impl InsertTask {
    /// Creates a new insert task.
    pub const fn new(receiver: Receiver, sender: Sender) -> Self {
        Self { receiver, sender }
    }
}

#[async_trait::async_trait]
impl EngineTask for InsertTask {
    type Error = InsertTaskError;

    async fn execute(&mut self) -> Result<(), Self::Error> {
        todo!()
    }
}
