//! The [Engine] is a task queue that receives and executes [EngineTask]s.

use super::{EngineTaskError, EngineTaskExt};
use crate::{EngineState, EngineTask};
use kona_derive::types::Signal;
use kona_protocol::L2BlockInfo;
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};
use tokio_util::sync::CancellationToken;

/// The [Engine] task queue.
///
/// Tasks are processed in FIFO order, providing synchronization and ordering guarantees
/// for the L2 execution layer and other actors. Because tasks are executed one at a time,
/// they are considered to be atomic operations over the [EngineState], and are given
/// exclusive access to the engine state during execution.
///
/// Tasks within the queue are also considered fallible. If they fail with a temporary error,
/// they are not popped from the queue and are retried on the next call to [Engine::drain].
#[derive(Debug)]
pub struct Engine {
    /// The state of the engine.
    state: EngineState,
    /// A sender that can be used to notify the engine actor of state changes.
    state_sender: tokio::sync::watch::Sender<EngineState>,
    /// The task queue.
    tasks: tokio::sync::mpsc::Receiver<EngineTask>,
    /// A way for the engine actor to signal back to the derivation actor
    /// if a block building task produced an `INVALID` response.
    derivation_signal_tx: UnboundedSender<Signal>,
    /// The channel to send the l2 safe head to the derivation actor.
    engine_l2_safe_head_tx: tokio::sync::watch::Sender<L2BlockInfo>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl Engine {
    /// Creates a new [Engine] with an empty task queue and the passed initial [EngineState].
    ///
    /// An initial [EngineTask::ForkchoiceUpdate] is added to the task queue to synchronize the
    /// engine with the forkchoice state of the [EngineState].
    pub const fn new(
        initial_state: EngineState,
        state_sender: tokio::sync::watch::Sender<EngineState>,
        task_receiver: tokio::sync::mpsc::Receiver<EngineTask>,
        derivation_signal_tx: UnboundedSender<Signal>,
        engine_l2_safe_head_tx: tokio::sync::watch::Sender<L2BlockInfo>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            state: initial_state,
            tasks: task_receiver,
            state_sender,
            derivation_signal_tx,
            engine_l2_safe_head_tx,
            cancellation,
        }
    }

    /// Returns the L2 Safe Head [`L2BlockInfo`] from the state.
    pub const fn safe_head(&self) -> L2BlockInfo {
        self.state.safe_head()
    }

    /// Returns a receiver that can be used to listen to engine state updates.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<EngineState> {
        self.state_sender.subscribe()
    }

    /// Starts the engine task queue.
    pub fn start(mut self) -> JoinHandle<Result<(), EngineError>> {
        let mut task_buf = Vec::with_capacity(self.tasks.capacity());

        tokio::spawn(async move {
            while self.tasks.recv_many(&mut task_buf, self.tasks.capacity()).await > 0 {
                let mut res = Ok(());

                for task in task_buf.drain(..) {
                    match task.execute(&mut self.state).await {
                        Ok(_) => {}
                        Err(EngineTaskError::Reset(e)) => {
                            res = Err(EngineTaskError::Reset(e));
                            break;
                        }
                        e => {
                            res = e;
                            break
                        }
                    }
                }

                match res {
                    Ok(_) => {
                        // Update the l2 safe head if needed.
                        let state_safe_head = self.safe_head();
                        let update = |head: &mut L2BlockInfo| {
                            if head != &state_safe_head {
                                *head = state_safe_head;
                                return true;
                            }
                            false
                        };
                        self.engine_l2_safe_head_tx.send_if_modified(update);
                        trace!(target: "engine", "Engine task queue drained.");
                    }
                    Err(EngineTaskError::Flush(e)) => {
                        // This error is encountered when the payload is marked INVALID
                        // by the engine api. Post-holocene, the payload is replaced by
                        // a "deposits-only" block and re-executed. At the same time,
                        // the channel and any remaining buffered batches are flushed.
                        warn!(target: "engine", ?e, "[HOLOCENE] Invalid payload, Flushing derivation pipeline.");
                        match self.derivation_signal_tx.send(Signal::FlushChannel) {
                            Ok(_) => {
                                debug!(target: "engine", "[SENT] flush signal to derivation actor")
                            }
                            Err(e) => {
                                error!(target: "engine", ?e, "[ENGINE] Failed to send flush signal to the derivation actor.");
                                self.cancellation.cancel();
                                return Err(EngineError::ChannelClosed);
                            }
                        }
                    }
                    Err(e) => warn!(target: "engine", ?e, "Error draining engine tasks"),
                }
            }

            info!(target: "engine", "Engine task receiver closed. Exiting engine task queue.");
            Ok(())
        })
    }
}

/// An error from the [`EngineActor`].
#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    /// Closed channel error.
    #[error("closed channel error")]
    ChannelClosed,
}
