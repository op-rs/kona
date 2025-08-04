use async_trait::async_trait;
use kona_supervisor_metrics::MetricsReporter;
use std::{io, sync::Arc, time::Duration};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::SupervisorActor;

#[derive(derive_more::Constructor)]
pub struct MetricWorker<R> {
    interval: Duration,
    // list of reporters
    reporters: Vec<Arc<R>>,
    cancel_token: CancellationToken,
}

#[async_trait]
impl<R> SupervisorActor for MetricWorker<R>
where
    R: MetricsReporter + Send + Sync + 'static,
{
    type InboundEvent = ();
    type Error = io::Error;

    async fn start(mut self) -> Result<(), Self::Error> {
        info!(
            target: "supervisor::metric_worker",
            "Starting MetricWorker with interval: {:?}",
            self.interval
        );

        let reporters = self.reporters;
        let interval = self.interval;

        loop {
            if self.cancel_token.is_cancelled() {
                info!("MetricReporter actor is stopping due to cancellation.");
                break;
            }

            for reporter in &reporters {
                reporter.report_metrics();
            }
            sleep(interval).await;
        }
        Ok(())
    }
}
