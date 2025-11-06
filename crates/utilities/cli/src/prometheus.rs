//! Utilities for spinning up a prometheus metrics server.

use metrics_exporter_prometheus::{BuildError, PrometheusBuilder};
use metrics_process::Collector;
use std::{
    net::{IpAddr, SocketAddr, TcpListener},
    thread::{self, sleep},
    time::Duration,
};
use tracing::info;

/// Start a Prometheus metrics server on the given port.
pub fn init_prometheus_server(addr: IpAddr, metrics_port: u16) -> Result<(), BuildError> {
    // If port is 0, we need to bind first to get the actual port assigned by the OS
    let actual_addr = if metrics_port == 0 {
        // Create a temporary listener to get the OS-assigned port
        let listener =
            TcpListener::bind((addr, 0)).expect("Failed to bind listener for metrics server");
        let bound_addr = listener.local_addr().expect("Failed to get local address from listener");
        // Close the temporary listener - PrometheusBuilder will create its own
        drop(listener);
        bound_addr
    } else {
        SocketAddr::from((addr, metrics_port))
    };

    let builder = PrometheusBuilder::new().with_http_listener(actual_addr);

    builder.install()?;

    // Initialise collector for system metrics e.g. CPU, memory, etc.
    let collector = Collector::default();
    collector.describe();

    thread::spawn(move || {
        loop {
            collector.collect();
            sleep(Duration::from_secs(60));
        }
    });

    info!(
        target: "prometheus",
        "Serving metrics at: http://{}",
        actual_addr
    );

    Ok(())
}
