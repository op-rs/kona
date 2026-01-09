use crate::{EngineError, EngineRpcRequest};
use derive_more::Constructor;
use kona_engine::{EngineClient, EngineState, RollupBoostServer};
use kona_genesis::RollupConfig;
use kona_rpc::RollupBoostAdminQuery;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

/// Processor for [`EngineRpcRequest`] requests.
#[derive(Constructor, Debug)]
pub struct EngineRpcProcessor<EngineClient_: EngineClient> {
    /// An [`EngineClient`] used for creating engine tasks.
    engine_client: Arc<EngineClient_>,
    // RollupBoost server handle
    rollup_boost_server: Arc<RollupBoostServer>,
    /// The [`RollupConfig`] used to build tasks.
    rollup_config: Arc<RollupConfig>,
    /// Receiver for [`EngineState`] updates.
    engine_state_receiver: watch::Receiver<EngineState>,
    /// Receiver for engine queue length updates.
    engine_queue_length_receiver: watch::Receiver<usize>,
}

impl<EngineClient_> EngineRpcProcessor<EngineClient_>
where
    EngineClient_: EngineClient + 'static,
{
    /// Starts a task to handle engine queries.
    pub fn start(
        self,
        mut request_channel: mpsc::Receiver<EngineRpcRequest>,
    ) -> JoinHandle<Result<(), EngineError>> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    query = request_channel.recv(), if !request_channel.is_closed() => {
                        let Some(query) = query else {
                            error!(target: "engine", "Engine rpc request receiver closed unexpectedly");
                            return Err(EngineError::ChannelClosed);
                        };
                        match query {
                            EngineRpcRequest::EngineQuery(req) => {
                                trace!(target: "engine", ?req, "Received engine query.");

                                if let Err(e) = req
                                    .handle(&self.engine_state_receiver, &self.engine_queue_length_receiver, &self.engine_client, &self.rollup_config)
                                    .await
                                {
                                    warn!(target: "engine", err = ?e, "Failed to handle engine query.");
                                }
                            },
                            EngineRpcRequest::RollupBoostAdminRequest(admin_query) => {
                                trace!(target: "engine", ?admin_query, "Received rollup boost admin query.");

                                self.handle_rollup_boost_admin_query(*admin_query);

                            },
                            EngineRpcRequest::RollupBoostHealthRequest(health_query) => {
                                trace!(target: "engine", ?health_query, "Received rollup boost health query.");

                                let health = self.rollup_boost_server.get_health();
                                health_query.sender.send(health.into()).unwrap();
                            },

                        }
                    }
                }
            }
        })
    }

    fn handle_rollup_boost_admin_query(&self, admin_query: RollupBoostAdminQuery) {
        match admin_query {
            RollupBoostAdminQuery::SetExecutionMode { execution_mode, sender } => {
                self.rollup_boost_server.server.set_execution_mode(execution_mode);
                let _ = sender.send(()).map_err(|_| {
                    warn!(target: "engine", "set execution mode response channel closed when trying to send");
                });
            }
            RollupBoostAdminQuery::GetExecutionMode { sender } => {
                let execution_mode = self.rollup_boost_server.server.get_execution_mode();
                let _ = sender.send(execution_mode).map_err(|_| {
                    warn!(target: "engine", "get execution mode response channel closed when trying to send");
                });
            }
        }
    }
}
