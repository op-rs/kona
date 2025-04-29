//! Discover Subcommand

use crate::flags::{GlobalArgs, MetricsArgs};
use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
use discv5::enr::CombinedKey;
use kona_p2p::{BootStore, Discv5Builder, LocalNode};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio_stream::StreamExt;
use tracing_appender::{non_blocking, non_blocking::WorkerGuard};

use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, LegendPosition},
};

/// The `discover` Subcommand
///
/// # Usage
///
/// ```sh
/// kona-node discover [FLAGS] [OPTIONS]
/// ```
#[derive(Parser, Debug, Clone)]
#[command(about = "Lists the OP Stack chains available in the superchain-registry")]
pub struct DiscoverCommand {
    /// Discovery port to listen on.
    #[arg(
        long,
        short = 'l',
        alias = "discovery-port",
        default_value = "9099",
        help = "Port to listen to discovery"
    )]
    pub disc_port: u16,
    /// The number of data points to display.
    /// Default is 20. If set to 0, all data points are displayed.
    #[arg(
        long,
        short = 'd',
        alias = "data-points",
        default_value = "20",
        help = "Number of data points to display"
    )]
    pub data_points: usize,
}

/// The Discovery TUI
#[derive(Debug)]
pub struct Discovery {
    /// If the application should quit.
    should_quit: bool,
    /// The chain id.
    l2_chain_id: u64,
    /// The discovery port to listen on.
    port: u16,
    /// Tuples of time since unix and the peer count at that timestamp.
    peers: Vec<(f64, f64)>,
    /// The list of timstamps.
    peer_window: [f64; 2],
    /// The valid peer count.
    valid_peers: Vec<(f64, f64)>,
    /// The list of timestamps.
    valid_peer_window: [f64; 2],
    /// Tuples of time since unix and the bootstore count at that timestamp.
    bootstore: Vec<(f64, f64)>,
    /// The list of timestamps.
    store_window: [f64; 2],
    /// The number of data points to display.
    /// Default is 20.
    data_points: usize,
}

impl Discovery {
    /// The number of frames to render per second.
    pub const FRAMES_PER_SECOND: f32 = 60.0;

    /// Returns the interval at which to re-render the TUI.
    pub fn render() -> tokio::time::Interval {
        let period = std::time::Duration::from_secs_f32(1.0 / Self::FRAMES_PER_SECOND);
        tokio::time::interval(period)
    }

    /// Instantiates a new [`Discovery`] TUI.
    pub fn new(l2_chain_id: u64, port: u16, data_points: usize) -> Self {
        let now = std::time::SystemTime::now();
        let now = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();

        Self {
            should_quit: false,
            l2_chain_id,
            port,
            peers: vec![],
            bootstore: vec![],
            peer_window: [now, now],
            store_window: [now, now],
            valid_peers: vec![],
            valid_peer_window: [now, now],
            data_points,
        }
    }

    /// Runs the main app.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> anyhow::Result<()> {
        let CombinedKey::Secp256k1(secret_key) = CombinedKey::generate_secp256k1() else {
            unreachable!()
        };
        let ip_and_port =
            LocalNode::new(secret_key, IpAddr::V4(Ipv4Addr::UNSPECIFIED), self.port, self.port);
        tracing::info!("Starting discovery service on {:?}", ip_and_port);
        let discovery_listening_address =
            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), self.port);
        let config = discv5::ConfigBuilder::new(discovery_listening_address.into())
            .ban_duration(Some(std::time::Duration::from_secs(30)))
            .build();

        let discovery_builder = Discv5Builder::new()
            .with_discovery_config(config)
            .with_interval(std::time::Duration::from_secs(2))
            .with_discovery_randomize(Some(std::time::Duration::from_secs(2)))
            .with_local_node(ip_and_port)
            .with_store_interval(std::time::Duration::from_secs(5))
            .with_chain_id(self.l2_chain_id)
            // Disable forwarding since gossip isn't important for this command.
            .disable_forward();
        let discovery = discovery_builder.build()?;
        let (handler, mut enr_receiver) = discovery.start();
        tracing::info!("Discovery service started, receiving peers.");

        let mut bootstore = BootStore::from_chain_id(self.l2_chain_id, None, vec![]);

        // Receive events from crossterm.
        let mut events = EventStream::new();

        // Print counts every second if available.
        let mut disc_ticker = tokio::time::interval(tokio::time::Duration::from_secs(1));
        let mut boot_ticker = tokio::time::interval(tokio::time::Duration::from_secs(5));
        let mut render = Self::render();
        while !self.should_quit {
            tokio::select! {
                _ = render.tick() => { terminal.draw(|frame| self.draw(frame))?; },
                enr = enr_receiver.recv() => {
                    match enr {
                        Some(enr) => {
                            tracing::info!("Received peer: {:?}", enr);
                        }
                        None => {
                            tracing::warn!("Failed to receive peer");
                        }
                    }
                }
                _ = disc_ticker.tick() => {
                    if let Ok(c) = handler.peer_count().try_recv() {
                        let now = std::time::SystemTime::now();
                        let since = now
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs_f64();
                        self.peers.push((since, c as f64));

                        if self.peers.len() > self.data_points && self.data_points != 0 {
                            self.peers.remove(0);
                        }
                        self.peer_window[0] = self.peers[0].0;
                        self.peer_window[1] = since;

                        if let Some(last) = self.bootstore.last() {
                            let mut last = *last;
                            last.0 = since;
                            self.bootstore.push(last);
                            if self.bootstore.len() > self.data_points && self.data_points != 0 {
                                self.bootstore.remove(0);
                            }
                            self.store_window[0] = self.bootstore[0].0;
                            self.store_window[1] = since;
                        }

                        tracing::debug!("Discovery peer count: {}", c);
                    }
                }
                _ = boot_ticker.tick() => {
                    let peers = bootstore.peers().len();
                    let valid_peers = bootstore.valid_peers_with_chain_id(self.l2_chain_id).len();
                    let now = std::time::SystemTime::now();
                    let since = now
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs_f64();
                    self.bootstore.push((since, peers as f64));
                    self.valid_peers.push((since, valid_peers as f64));

                    if self.bootstore.len() > self.data_points && self.data_points != 0 {
                        self.bootstore.remove(0);
                    }
                    self.store_window[0] = self.bootstore[0].0;
                    self.store_window[1] = since;

                    if self.valid_peers.len() > self.data_points && self.data_points != 0 {
                        self.valid_peers.remove(0);
                    }
                    self.valid_peer_window[0] = self.valid_peers[0].0;
                    self.valid_peer_window[1] = since;

                    if let Some(last) = self.peers.last() {
                        let mut last = *last;
                        last.0 = since;
                        self.peers.push(last);
                        if self.peers.len() > self.data_points && self.data_points != 0 {
                            self.peers.remove(0);
                        }
                        self.peer_window[0] = self.peers[0].0;
                        self.peer_window[1] = since;
                    }
                    tracing::debug!("Bootstore peer list: {}", peers);
                }
                Some(Ok(event)) = events.next() => self.handle_event(&event),
            }
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame<'_>) {
        let [top, bottom] = Layout::vertical([Constraint::Fill(1); 2]).areas(frame.area());
        let [peers, store, valids] = Layout::horizontal([Constraint::Fill(1); 3]).areas(top);
        let [logs] = Layout::horizontal([Constraint::Fill(1)]).areas(bottom);

        self.render_peer_chart(frame, peers);
        self.render_store_chart(frame, store);
        self.render_valid_chart(frame, valids);
        self.render_logs(frame, logs);
    }

    fn render_logs(&self, frame: &mut Frame<'_>, area: Rect) {
        let Some(home) = dirs::home_dir() else {
            tracing::warn!("Failed to get home directory");
            return;
        };
        let log_file = home.join(".kona").join("tracing.log");
        let log_lines =
            std::fs::read_to_string(&log_file).unwrap_or_else(|_| "No logs available".to_string());
        let log_lines: Vec<Line<'_>> =
            log_lines.lines().map(|line| Line::from(Span::raw(line))).rev().collect();
        let log_lines = log_lines.into_iter().take(20).collect::<Vec<_>>();
        let paragraph = ratatui::widgets::Paragraph::new(log_lines)
            .block(Block::default().title("Logs").borders(ratatui::widgets::Borders::ALL))
            .wrap(ratatui::widgets::Wrap { trim: true })
            .scroll((0, 0));
        frame.render_widget(ratatui::widgets::Clear, area);
        frame.render_widget(paragraph, area);
    }

    fn render_peer_chart(&self, frame: &mut Frame<'_>, area: Rect) {
        let x_labels = vec![
            Span::styled(
                format!("{:.0}", self.peer_window[0]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{:.0}", (self.peer_window[0] + self.peer_window[1]) / 2.0)),
            Span::styled(
                format!("{:.0}", self.peer_window[1]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        let datasets = vec![
            Dataset::default()
                .name("Discv5 Peer Count")
                .marker(symbols::Marker::Dot)
                .style(Style::default().fg(Color::Cyan))
                .data(&self.peers),
        ];

        let mut y_max = self.peers.iter().map(|(_, y)| *y).fold(0.0, f64::max);

        // Add a 20% margin to the max y value
        y_max = (y_max * 1.2).ceil();

        let chart = Chart::new(datasets)
            .block(Block::bordered().title("Discovery Peer Count"))
            .x_axis(
                Axis::default()
                    .title("X Axis")
                    .style(Style::default().fg(Color::Gray))
                    .labels(x_labels)
                    .bounds(self.peer_window),
            )
            .y_axis(
                Axis::default()
                    .title("Y Axis")
                    .style(Style::default().fg(Color::Gray))
                    .labels(["0".bold(), format!("{}", y_max).bold()])
                    .bounds([0.0, y_max]),
            );

        frame.render_widget(chart, area);
    }

    fn render_store_chart(&self, frame: &mut Frame<'_>, area: Rect) {
        let x_labels = vec![
            Span::styled(
                format!("{:.0}", self.store_window[0]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{:.0}", (self.store_window[0] + self.store_window[1]) / 2.0)),
            Span::styled(
                format!("{:.0}", self.store_window[1]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        let datasets = vec![
            Dataset::default()
                .name("Bootstore Peer Count")
                .marker(symbols::Marker::Dot)
                .style(Style::default().fg(Color::Yellow))
                .data(&self.bootstore),
        ];

        let y_min = self.bootstore.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);

        let mut y_max = self.bootstore.iter().map(|(_, y)| *y).fold(0.0, f64::max);

        // Add the max of a 20% of the difference between min and max or 10 to the max y value
        let diff = y_max - y_min;
        if diff < 10.0 {
            y_max += 10.0;
        } else {
            y_max += (diff * 0.2).ceil();
        }

        let chart = Chart::new(datasets)
            .block(Block::bordered().title("Bootstore Peer Count"))
            .legend_position(Some(LegendPosition::TopRight))
            .hidden_legend_constraints((Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)))
            .x_axis(
                Axis::default()
                    .title("X Axis")
                    .style(Style::default().fg(Color::Gray))
                    .labels(x_labels)
                    .bounds(self.store_window),
            )
            .y_axis(
                Axis::default()
                    .title("Y Axis")
                    .style(Style::default().fg(Color::Gray))
                    .labels([format!("{}", y_min).bold(), format!("{}", y_max).bold()])
                    .bounds([y_min, y_max]),
            );

        frame.render_widget(chart, area);
    }

    fn render_valid_chart(&self, frame: &mut Frame<'_>, area: Rect) {
        let x_labels = vec![
            Span::styled(
                format!("{:.0}", self.valid_peer_window[0]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{:.0}",
                (self.valid_peer_window[0] + self.valid_peer_window[1]) / 2.0
            )),
            Span::styled(
                format!("{:.0}", self.valid_peer_window[1]),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ];
        let datasets = vec![
            Dataset::default()
                .name("Valid Peer Count")
                .marker(symbols::Marker::Dot)
                .style(Style::default().fg(Color::Yellow))
                .data(&self.valid_peers),
        ];

        let y_min = self.valid_peers.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);
        let mut y_max = self.valid_peers.iter().map(|(_, y)| *y).fold(0.0, f64::max);

        // Add the max of a 20% of the difference between min and max or 10 to the max y value
        let diff = y_max - y_min;
        if diff < 10.0 {
            y_max += 10.0;
        } else {
            y_max += (diff * 0.2).ceil();
        }

        let chart = Chart::new(datasets)
            .block(Block::bordered().title("Valid Peer Count"))
            .legend_position(Some(LegendPosition::TopRight))
            .hidden_legend_constraints((Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)))
            .x_axis(
                Axis::default()
                    .title("X Axis")
                    .style(Style::default().fg(Color::Gray))
                    .labels(x_labels)
                    .bounds(self.valid_peer_window),
            )
            .y_axis(
                Axis::default()
                    .title("Y Axis")
                    .style(Style::default().fg(Color::Gray))
                    .labels([format!("{}", y_min).bold(), format!("{}", y_max).bold()])
                    .bounds([y_min, y_max]),
            );

        frame.render_widget(chart, area);
    }

    fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                    _ => {}
                }
            }
        }
    }
}

impl DiscoverCommand {
    /// Initializes the telemetry stack and Prometheus metrics recorder.
    pub fn init_telemetry(&self, _args: &GlobalArgs, metrics: &MetricsArgs) -> anyhow::Result<()> {
        metrics.init_metrics()
    }

    /// Initializes tracing for the application.
    pub fn init_tracing(&self, args: &GlobalArgs) -> anyhow::Result<WorkerGuard> {
        let level = match args.v {
            0 => tracing::Level::INFO,
            1 => tracing::Level::DEBUG,
            _ => tracing::Level::TRACE,
        };
        // Create the ~/.kona directory if it doesn't exist.
        let kona_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?
            .join(".kona");
        if !kona_dir.exists() {
            std::fs::create_dir_all(&kona_dir)
                .map_err(|e| anyhow::anyhow!("Failed to create ~/.kona directory: {:?}", e))?;
        }
        // Create the tracing log file in the ~/.kona directory.
        let log_file = kona_dir.join("tracing.log");
        if log_file.exists() {
            std::fs::remove_file(&log_file).map_err(|e| {
                anyhow::anyhow!("Failed to remove existing tracing log file: {:?}", e)
            })?;
        }
        let file = std::fs::File::create(&log_file)
            .map_err(|e| anyhow::anyhow!("Failed to create tracing log file: {:?}", e))?;
        let (non_blocking, guard) = non_blocking(file);
        let env_filter = tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(level.into())
            .add_directive("discv5=error".parse()?)
            .add_directive("tokio=error".parse()?);

        tracing_subscriber::fmt()
            .with_writer(non_blocking)
            .with_max_level(level)
            .with_env_filter(env_filter)
            .init();
        Ok(guard)
    }

    /// Runs the subcommand.
    pub async fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        let _guard = self.init_tracing(args)?;
        color_eyre::install().map_err(|e| anyhow::anyhow!(e))?;
        let terminal = ratatui::init();
        let res =
            Discovery::new(args.l2_chain_id, self.disc_port, self.data_points).run(terminal).await;
        ratatui::restore();
        res
    }
}
