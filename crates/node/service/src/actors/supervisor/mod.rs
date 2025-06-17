//! Contains types and traits for the supervisor rpc actor.

mod traits;
pub use traits::SupervisorExt;

mod actor;
pub use actor::{SupervisorActor, SupervisorActorError};

mod ext;
pub use ext::SupervisorRpcServerExt;
