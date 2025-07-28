use kona_interop::DerivedRefPair;
use tokio::sync::RwLock;

/// This module contains the state management for the chain processor.
/// It provides a way to track the invalidated blocks and manage the state of the chain processor
#[derive(Debug)]
pub struct ProcessorState {
    invalidated_block: RwLock<Option<DerivedRefPair>>,
}

impl ProcessorState {
    /// Creates a new instance of [`ProcessorState`].
    pub fn new() -> Self {
        Self { invalidated_block: RwLock::new(None) }
    }

    /// Returns `true` if the state is invalidated, otherwise `false`.
    pub async fn is_invalidated(&self) -> bool {
        self.invalidated_block.read().await.is_some()
    }

    /// Returns the invalidated block if it exists.
    pub async fn get_invalidated(&self) -> Option<DerivedRefPair> {
        self.invalidated_block.read().await.clone()
    }

    /// Sets the invalidated block to the given pair if it is not already set.
    pub async fn set_invalidated(&self, pair: DerivedRefPair) -> bool {
        let mut guard = self.invalidated_block.write().await;
        if guard.is_some() {
            false // Already set
        } else {
            *guard = Some(pair);
            true
        }
    }

    /// Clears the invalidated block.
    pub async fn clear_invalidated(&self) {
        *self.invalidated_block.write().await = None;
    }
}
