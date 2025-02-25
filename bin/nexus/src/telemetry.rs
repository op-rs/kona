use kona_cli::init_tracing_subscriber;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Initialize the tracing stack and Prometheus metrics recorder.
///
/// This function should be called at the beginning of the program.
pub fn init_stack(verbosity: u8, metrics_port: u16) -> anyhow::Result<()> {
    let filter = EnvFilter::builder().with_default_directive("hilo=info".parse()?).from_env_lossy();
    init_tracing_subscriber(verbosity, Some(filter))?;

    let prometheus_addr = SocketAddr::from(([0, 0, 0, 0], metrics_port));
    let builder = PrometheusBuilder::new().with_http_listener(prometheus_addr);

    if let Err(e) = builder.install() {
        anyhow::bail!("failed to install Prometheus recorder: {:?}", e);
    } else {
        info!("Telemetry initialized. Serving Prometheus metrics at: http://{}", prometheus_addr);
    }

    Ok(())
}
