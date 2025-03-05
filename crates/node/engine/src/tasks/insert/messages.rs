//! Contains the message types for the insert task.

/// An inbound message from an external actor to the [crate::InsertTask].
#[derive(Debug, Clone)]
pub enum InsertTaskInput {}

/// An outbound message from the [crate::InsertTask] to an external actor.
#[derive(Debug, Clone)]
pub enum InsertTaskOut {
    /// Request the sync status.
    SyncStatus,
}
