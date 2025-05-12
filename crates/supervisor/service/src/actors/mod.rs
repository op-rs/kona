//! [SupervisorActor] services for the supervisor.
//!
//! [SupervisorActor]: super::SupervisorActor

mod traits;
pub use traits::SupervisorActor;

mod l1_watcher_rpc;
pub use l1_watcher_rpc::{L1WatcherRpc, L1WatcherRpcError};
