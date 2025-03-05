use kona_cli::{init_prometheus_server, init_tracing_subscriber};
use tracing_subscriber::EnvFilter;

/// Initialize the tracing stack and Prometheus metrics recorder.
///
/// This function should be called at the beginning of the program.
pub fn init_stack(verbosity: u8, metrics_port: u16) -> anyhow::Result<()> {
    // Initialize the tracing subscriber.
    init_tracing_subscriber(verbosity, None::<EnvFilter>)?;

    // Start the Prometheus metrics server.
    init_prometheus_server(metrics_port)?;

    Ok(())
}
