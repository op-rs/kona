//! Utilities for the rollup node service, internal to the crate.

/// Spawns a set of parallel actors in a [`JoinSet`], and cancels all actors if any of them fail.
/// The type of the error in the [`NodeActor`]s is erased to avoid having to specify a common error
/// type between actors.
///
/// Actors are passed in as optional arguments, in case a given actor is not needed.
///
/// [`JoinSet`]: tokio::task::JoinSet
/// [`NodeActor`]: crate::NodeActor
macro_rules! spawn_and_wait {
    ($cancellation:expr, actors = [$($actor:expr$(,)?)*]) => {
        let mut task_handles = tokio::task::JoinSet::new();

        // Check if the actor is present, and spawn it if it is.
        $(
            if let Some(actor) = $actor {
                task_handles.spawn(async move {
                    if let Err(e) = actor.start().await {
                        tracing::error!(target: "rollup_node", "{e}");
                        return Err(Self::Error::from(e));
                    }
                    Ok(())
                });
            }
        )*

        while let Some(result) = task_handles.join_next().await {
            if let Err(e) = result {
                tracing::error!(target: "rollup_node", "Critical error in sub-routine: {e}");

                // Cancel all tasks and gracefully shutdown.
                $cancellation.cancel();
            }
        }
    };
}

// Export the `spawn_and_wait` macro for use in other modules.
pub(crate) use spawn_and_wait;
