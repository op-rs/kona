use async_trait::async_trait;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::SupervisorActor;

pub struct MetricReporter<R> {
    interval: Duration,
    // list of reporters
    reporters: Vec<Arc<R>>,
    cancel_token: CancellationToken,
}

impl<R> MetricReporter<R>
where
    R: kona_supervisor_metrics::MetricsReporter + Send + Sync + 'static,
{
    pub fn new(
        interval: Duration,
        reporters: Vec<Arc<R>>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self { interval, reporters, cancel_token }
    }
}

#[async_trait]
impl<R> SupervisorActor for MetricReporter<R>
where
    R: kona_supervisor_metrics::MetricsReporter + Send + Sync + 'static,
{
    type InboundEvent = ();
    type Error = std::io::Error;

    async fn start(self) -> Result<(), Self::Error> {
        let reporters = self.reporters;
        let interval = self.interval;

        tokio::spawn(async move {
            loop {
                if self.cancel_token.is_cancelled() {
                    tracing::info!("MetricReporter actor is stopping due to cancellation.");
                    break;
                }

                for reporter in &reporters {
                    reporter.report_metrics();
                }
                sleep(interval).await;
            }
        });

        Ok(())
    }
}
