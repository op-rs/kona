//! L1 Watcher Module
//! This module provides functionality for watching L1 blocks and managing subscriptions to L1
//! events. It also handles L1 reorgs for all chains in the dependency set.
mod reorg;
mod watcher;

pub use reorg::ReorgHandler;
pub use watcher::L1Watcher;
