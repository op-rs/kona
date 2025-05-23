//! Utilities for the rollup node service, internal to the crate.

/// Spawns a set of parallel actors in a [JoinSet], and cancels all actors if any of them fail. The
/// type of the error in the [NodeActor]s is erased to avoid having to specify a common error type
/// between actors.
///
/// Actors are passed in as optional arguments, in case a given actor is not needed.
///
/// [JoinSet]: tokio::task::JoinSet
/// [NodeActor]: crate::NodeActor
macro_rules! spawn_and_wait {
    ($($actor:expr$(,)?)*) => {
        // Check if the actor is present, and spawn it if it is.
        $(
            if let Some(actor) = $actor {
                tokio::spawn
                (
                async move {
                    if let Err(e) = actor.start().await {
                        return Err(format!("{e:?}"));
                    }
                    Ok(())
                });
            }
        )*
    };
}

// Export the `spawn_and_wait` macro for use in other modules.
pub(crate) use spawn_and_wait;
