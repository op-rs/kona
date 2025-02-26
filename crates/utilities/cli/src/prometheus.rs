//! Utilities for spinning up a prometheus metrics server.

use metrics_exporter_prometheus::{BuildError, PrometheusBuilder};
use std::net::SocketAddr;
use tracing::info;

/// Start a Prometheus metrics server on the given port.
pub fn init_prometheus_server(metrics_port: u16) -> Result<(), BuildError> {
    let prometheus_addr = SocketAddr::from(([0, 0, 0, 0], metrics_port));
    let builder = PrometheusBuilder::new().with_http_listener(prometheus_addr);

    builder.install()?;
    info!(
        target: "prometheus",
        "Metrics server initialized. Serving Prometheus metrics at: http://{}",
        prometheus_addr
    );

    Ok(())
}
