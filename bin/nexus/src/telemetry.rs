use std::{io::IsTerminal, net::SocketAddr};

use metrics_exporter_prometheus::PrometheusBuilder;
use tracing::{info, Level};
use tracing_subscriber::{
    fmt::Layer as FmtLayer, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

/// Initialize the tracing stack and Prometheus metrics recorder.
///
/// This function should be called at the beginning of the program.
pub fn init_stack(metrics_port: u16) -> anyhow::Result<()> {
    let filter = EnvFilter::builder().with_default_directive("hilo=info".parse()?).from_env_lossy();

    // Whether to use ANSI formatting and colors in the console output.
    // If unset, always use colors if stdout is a tty.
    // If set to "never", just disable all colors.
    let should_use_colors = match std::env::var("RUST_LOG_STYLE") {
        Ok(val) => val != "never",
        Err(_) => std::io::stdout().is_terminal(),
    };

    // Whether to show the tracing target in the console output.
    // If set, always show the target unless explicitly set to "0".
    // If unset, show target only if the filter is more verbose than INFO.
    let should_show_target = match std::env::var("RUST_LOG_TARGET") {
        Ok(val) => val != "0",
        Err(_) => filter.max_level_hint().map_or(true, |max_level| max_level > Level::INFO),
    };

    let std_layer = FmtLayer::new()
        .with_ansi(should_use_colors)
        .with_target(should_show_target)
        .with_writer(std::io::stdout)
        .with_filter(filter);

    tracing_subscriber::registry().with(std_layer).try_init()?;

    let prometheus_addr = SocketAddr::from(([0, 0, 0, 0], metrics_port));
    let builder = PrometheusBuilder::new().with_http_listener(prometheus_addr);

    if let Err(e) = builder.install() {
        anyhow::bail!("failed to install Prometheus recorder: {:?}", e);
    } else {
        info!("Telemetry initialized. Serving Prometheus metrics at: http://{}", prometheus_addr);
    }

    Ok(())
}
