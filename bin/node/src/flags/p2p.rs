//! P2P CLI Flags
//!
//! These are based on p2p flags from the [`op-node`][op-node] CLI.
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/flags/p2p_flags.go

use crate::flags::GlobalArgs;
use alloy_primitives::B256;
use anyhow::Result;
use clap::Parser;
use discv5::{Enr, enr::k256};
use kona_genesis::RollupConfig;
use kona_p2p::{Config, LocalNode};
use kona_peers::{PeerMonitoring, PeerScoreLevel};
use kona_sources::RuntimeLoader;
use libp2p::identity::Keypair;
use std::{
    net::{IpAddr, SocketAddr},
    num::ParseIntError,
    path::PathBuf,
    sync::Arc,
};
use tokio::time::Duration;
use url::Url;

/// P2P CLI Flags
#[derive(Parser, Clone, Debug, PartialEq, Eq)]
pub struct P2PArgs {
    /// Fully disable the P2P stack.
    #[arg(long = "p2p.disable", default_value = "false", env = "KONA_NODE_P2P_DISABLE")]
    pub disabled: bool,
    /// Disable Discv5 (node discovery).
    #[arg(long = "p2p.no-discovery", default_value = "false", env = "KONA_NODE_P2P_NO_DISCOVERY")]
    pub no_discovery: bool,
    /// Read the hex-encoded 32-byte private key for the peer ID from this txt file.
    /// Created if not already exists. Important to persist to keep the same network identity after
    /// restarting, maintaining the previous advertised identity.
    #[arg(long = "p2p.priv.path", env = "KONA_NODE_P2P_PRIV_PATH")]
    pub priv_path: Option<PathBuf>,
    /// The hex-encoded 32-byte private key for the peer ID.
    #[arg(long = "p2p.priv.raw", env = "KONA_NODE_P2P_PRIV_RAW")]
    pub private_key: Option<B256>,

    /// IP to advertise to external peers from Discv5.
    /// Optional argument. Use the `p2p.listen.ip` if not set.
    ///
    /// Technical note: if this argument is set, the dynamic ENR updates from the discovery layer
    /// will be disabled. This is to allow the advertised IP to be static (to use in a network
    /// behind a NAT for instance).
    #[arg(long = "p2p.advertise.ip", env = "KONA_NODE_P2P_ADVERTISE_IP")]
    pub advertise_ip: Option<IpAddr>,
    /// TCP port to advertise to external peers from the discovery layer. Same as `p2p.listen.tcp`
    /// if set to zero.
    #[arg(
        long = "p2p.advertise.tcp",
        default_value = "0",
        env = "KONA_NODE_P2P_ADVERTISE_TCP_PORT"
    )]
    pub advertise_tcp_port: u16,
    /// UDP port to advertise to external peers from the discovery layer.
    /// Same as `p2p.listen.udp` if set to zero.
    #[arg(
        long = "p2p.advertise.udp",
        default_value = "0",
        env = "KONA_NODE_P2P_ADVERTISE_UDP_PORT"
    )]
    pub advertise_udp_port: u16,

    /// IP to bind LibP2P/Discv5 to.
    #[arg(long = "p2p.listen.ip", default_value = "0.0.0.0", env = "KONA_NODE_P2P_LISTEN_IP")]
    pub listen_ip: IpAddr,
    /// TCP port to bind LibP2P to. Any available system port if set to 0.
    #[arg(long = "p2p.listen.tcp", default_value = "9222", env = "KONA_NODE_P2P_LISTEN_TCP_PORT")]
    pub listen_tcp_port: u16,
    /// UDP port to bind Discv5 to. Same as TCP port if left 0.
    #[arg(long = "p2p.listen.udp", default_value = "9223", env = "KONA_NODE_P2P_LISTEN_UDP_PORT")]
    pub listen_udp_port: u16,
    /// Low-tide peer count. The node actively searches for new peer connections if below this
    /// amount.
    #[arg(long = "p2p.peers.lo", default_value = "20", env = "KONA_NODE_P2P_PEERS_LO")]
    pub peers_lo: u32,
    /// High-tide peer count. The node starts pruning peer connections slowly after reaching this
    /// number.
    #[arg(long = "p2p.peers.hi", default_value = "30", env = "KONA_NODE_P2P_PEERS_HI")]
    pub peers_hi: u32,
    /// Grace period to keep a newly connected peer around, if it is not misbehaving.
    #[arg(
        long = "p2p.peers.grace",
        default_value = "30",
        env = "KONA_NODE_P2P_PEERS_GRACE",
        value_parser = |arg: &str| -> Result<Duration, ParseIntError> {Ok(Duration::from_secs(arg.parse()?))}
    )]
    pub peers_grace: Duration,
    /// Configure GossipSub topic stable mesh target count.
    /// Aka: The desired outbound degree (numbers of peers to gossip to).
    #[arg(long = "p2p.gossip.mesh.d", default_value = "8", env = "KONA_NODE_P2P_GOSSIP_MESH_D")]
    pub gossip_mesh_d: usize,
    /// Configure GossipSub topic stable mesh low watermark.
    /// Aka: The lower bound of outbound degree.
    #[arg(long = "p2p.gossip.mesh.lo", default_value = "6", env = "KONA_NODE_P2P_GOSSIP_MESH_DLO")]
    pub gossip_mesh_dlo: usize,
    /// Configure GossipSub topic stable mesh high watermark.
    /// Aka: The upper bound of outbound degree (additional peers will not receive gossip).
    #[arg(
        long = "p2p.gossip.mesh.dhi",
        default_value = "12",
        env = "KONA_NODE_P2P_GOSSIP_MESH_DHI"
    )]
    pub gossip_mesh_dhi: usize,
    /// Configure GossipSub gossip target.
    /// Aka: The target degree for gossip only (not messaging like p2p.gossip.mesh.d, just
    /// announcements of IHAVE).
    #[arg(
        long = "p2p.gossip.mesh.dlazy",
        default_value = "6",
        env = "KONA_NODE_P2P_GOSSIP_MESH_DLAZY"
    )]
    pub gossip_mesh_dlazy: usize,
    /// Configure GossipSub to publish messages to all known peers on the topic, outside of the
    /// mesh. Also see Dlazy as less aggressive alternative.
    #[arg(
        long = "p2p.gossip.mesh.floodpublish",
        default_value = "false",
        env = "KONA_NODE_P2P_GOSSIP_FLOOD_PUBLISH"
    )]
    pub gossip_flood_publish: bool,
    /// Sets the peer scoring strategy for the P2P stack.
    /// Can be one of: none or light.
    ///
    /// TODO(@theochap, `<https://github.com/op-rs/kona/issues/1855>`): By default, the P2P stack is configured to not score peers.
    #[arg(long = "p2p.scoring", default_value = "off", env = "KONA_NODE_P2P_SCORING")]
    pub scoring: PeerScoreLevel,

    /// Allows to ban peers based on their score.
    ///
    /// Peers are banned based on a ban threshold (see `p2p.ban.threshold`).
    /// If a peer's score is below the threshold, it gets automatically banned.
    #[arg(long = "p2p.ban.peers", default_value = "false", env = "KONA_NODE_P2P_BAN_PEERS")]
    pub ban_enabled: bool,

    /// The threshold used to ban peers.
    ///
    /// For peers to be banned, the `p2p.ban.peers` flag must be set to `true`.
    /// By default, peers are banned if their score is below -100. This follows the `op-node` default `<https://github.com/ethereum-optimism/optimism/blob/09a8351a72e43647c8a96f98c16bb60e7b25dc6e/op-node/flags/p2p_flags.go#L123-L130>`.
    #[arg(long = "p2p.ban.threshold", default_value = "-100", env = "KONA_NODE_P2P_BAN_THRESHOLD")]
    pub ban_threshold: i64,

    /// The duration in minutes to ban a peer for.
    ///
    /// For peers to be banned, the `p2p.ban.peers` flag must be set to `true`.
    /// By default peers are banned for 1 hour. This follows the `op-node` default `<https://github.com/ethereum-optimism/optimism/blob/09a8351a72e43647c8a96f98c16bb60e7b25dc6e/op-node/flags/p2p_flags.go#L131-L138>`.
    #[arg(long = "p2p.ban.duration", default_value = "60", env = "KONA_NODE_P2P_BAN_DURATION")]
    pub ban_duration: u64,

    /// The interval in seconds to find peers using the discovery service.
    /// Defaults to 5 seconds.
    #[arg(
        long = "p2p.discovery.interval",
        default_value = "5",
        env = "KONA_NODE_P2P_DISCOVERY_INTERVAL"
    )]
    pub discovery_interval: u64,
    /// The directory to store the bootstore.
    #[arg(long = "p2p.bootstore", env = "KONA_NODE_P2P_BOOTSTORE")]
    pub bootstore: Option<PathBuf>,
    /// Peer Redialing threshold is the maximum amount of times to attempt to redial a peer that
    /// disconnects. By default, peers are *not* redialed. If set to 0, the peer will be
    /// redialed indefinitely.
    #[arg(long = "p2p.redial", env = "KONA_NODE_P2P_REDIAL", default_value = "500")]
    pub peer_redial: Option<u64>,

    /// An optional list of bootnode ENRs to start the node with.
    #[arg(long = "p2p.bootnodes", value_delimiter = ',', env = "KONA_NODE_P2P_BOOTNODES")]
    pub bootnodes: Vec<Enr>,

    /// Optionally enable topic scoring.
    ///
    /// Topic scoring is a mechanism to score peers based on their behavior in the gossip network.
    /// Historically, topic scoring was only enabled for the v1 topic on the OP Stack p2p network
    /// in the `op-node`. This was a silent bug, and topic scoring is actively being
    /// [phased out of the `op-node`][out].
    ///
    /// This flag is only presented for backwards compatibility and debugging purposes.
    ///
    /// [out]: https://github.com/ethereum-optimism/optimism/pull/15719
    #[arg(
        long = "p2p.topic-scoring",
        default_value = "false",
        env = "KONA_NODE_P2P_TOPIC_SCORING"
    )]
    pub topic_scoring: bool,

    /// An optional unsafe block signer address.
    ///
    /// By default, this is fetched from the chain config in the superchain-registry using the
    /// specified L2 chain ID.
    #[arg(long = "p2p.unsafe.block.signer", env = "KONA_NODE_P2P_UNSAFE_BLOCK_SIGNER")]
    pub unsafe_block_signer: Option<alloy_primitives::Address>,

    /// An optional flag to remove random peers from discovery to rotate the peer set.
    ///
    /// This is the number of seconds to wait before removing a peer from the discovery
    /// service. By default, peers are not removed from the discovery service.
    ///
    /// This is useful for discovering a wider set of peers.
    #[arg(long = "p2p.discovery.randomize", env = "KONA_NODE_P2P_DISCOVERY_RANDOMIZE")]
    pub discovery_randomize: Option<u64>,
}

impl Default for P2PArgs {
    fn default() -> Self {
        // Construct default values using the clap parser.
        // This works since none of the cli flags are required.
        Self::parse_from::<[_; 0], &str>([])
    }
}

impl P2PArgs {
    fn check_ports_inner(ip_addr: IpAddr, tcp_port: u16, udp_port: u16) -> Result<()> {
        if tcp_port == 0 {
            return Ok(());
        }
        if udp_port == 0 {
            return Ok(());
        }
        let tcp_socket = std::net::TcpListener::bind((ip_addr, tcp_port));
        let udp_socket = std::net::UdpSocket::bind((ip_addr, udp_port));
        if tcp_socket.is_err() {
            tracing::error!(target: "p2p::flags", "TCP port {} is already in use", tcp_port);
            tracing::warn!(target: "p2p::flags", "Specify a different TCP port with --p2p.listen.tcp");
            anyhow::bail!("TCP port {} is already in use", tcp_port);
        }
        if udp_socket.is_err() {
            tracing::error!(target: "p2p::flags", "UDP port {} is already in use", udp_port);
            tracing::warn!(target: "p2p::flags", "Specify a different UDP port with --p2p.listen.udp");
            anyhow::bail!("UDP port {} is already in use", udp_port);
        }

        Ok(())
    }

    /// Checks if the ports are available on the system.
    ///
    /// If either of the ports are `0`, this check is skipped.
    ///
    /// ## Errors
    ///
    /// - If the TCP port is already in use.
    /// - If the UDP port is already in use.
    pub fn check_ports(&self) -> Result<()> {
        if self.disabled {
            tracing::debug!(target: "p2p::flags", "P2P is disabled, skipping port check");
            return Ok(());
        }
        Self::check_ports_inner(
            // If the advertised ip is not specified, we use the listen ip.
            self.advertise_ip.unwrap_or(self.listen_ip),
            self.advertise_tcp_port,
            self.advertise_udp_port,
        )?;
        Self::check_ports_inner(self.listen_ip, self.listen_tcp_port, self.listen_udp_port)?;

        Ok(())
    }

    /// Returns the [`discv5::Config`] from the CLI arguments.
    pub fn discv5_config(
        &self,
        listen_config: discv5::ListenConfig,
        static_ip: bool,
    ) -> discv5::Config {
        // We can use a default listen config here since it
        // will be overridden by the discovery service builder.
        let mut builder = discv5::ConfigBuilder::new(listen_config);

        if static_ip {
            builder.disable_enr_update();

            // If we have a static IP, we don't want to use any kind of NAT discovery mechanism.
            builder.auto_nat_listen_duration(None);
        }

        builder.build()
    }

    /// Returns the unsafe block signer from the CLI arguments.
    pub async fn unsafe_block_signer(
        &self,
        config: &RollupConfig,
        args: &GlobalArgs,
        l1_rpc: Option<Url>,
    ) -> anyhow::Result<alloy_primitives::Address> {
        // First attempt to load the unsafe block signer from the runtime loader.
        if let Some(url) = l1_rpc {
            let mut loader = RuntimeLoader::new(url, Arc::new(config.clone()));
            let runtime = loader
                .load_latest()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to load runtime: {}", e))?;
            return Ok(runtime.unsafe_block_signer_address);
        }

        // Otherwise use the genesis signer or the configured unsafe block signer.
        args.genesis_signer().or_else(|_| {
            self.unsafe_block_signer.ok_or(anyhow::anyhow!("Unsafe block signer not provided"))
        })
    }

    /// Constructs kona's P2P network [`Config`] from CLI arguments.
    ///
    /// ## Parameters
    ///
    /// - [`GlobalArgs`]: required to fetch the genesis unsafe block signer.
    ///
    /// Errors if the genesis unsafe block signer isn't available for the specified L2 Chain ID.
    pub async fn config(
        self,
        config: &RollupConfig,
        args: &GlobalArgs,
        l1_rpc: Option<Url>,
    ) -> anyhow::Result<Config> {
        // Note: the advertised address is contained in the ENR for external peers from the
        // discovery layer to use.

        // Fallback to the listen ip if the advertise ip is not specified
        let advertise_ip = self.advertise_ip.unwrap_or(self.listen_ip);

        // If the advertise ip is set, we will disable the dynamic ENR updates.
        let static_ip = self.advertise_ip.is_some();

        // If the advertise tcp port is null, use the listen tcp port
        let advertise_tcp_port = if self.advertise_tcp_port != 0 {
            self.advertise_tcp_port
        } else {
            self.listen_tcp_port
        };

        let advertise_udp_port = if self.advertise_udp_port != 0 {
            self.advertise_udp_port
        } else {
            self.listen_udp_port
        };

        let keypair = self.keypair().unwrap_or_else(|_| Keypair::generate_secp256k1());
        let secp256k1_key = keypair.clone().try_into_secp256k1()
            .map_err(|e| anyhow::anyhow!("Impossible to convert keypair to secp256k1. This is a bug since we only support secp256k1 keys: {e}"))?
            .secret().to_bytes();
        let local_node_key = k256::ecdsa::SigningKey::from_bytes(&secp256k1_key.into())
            .map_err(|e| anyhow::anyhow!("Impossible to convert keypair to k256 signing key. This is a bug since we only support secp256k1 keys: {e}"))?;

        let discovery_address =
            LocalNode::new(local_node_key, advertise_ip, advertise_tcp_port, advertise_udp_port);
        let gossip_config = kona_p2p::default_config_builder()
            .mesh_n(self.gossip_mesh_d)
            .mesh_n_low(self.gossip_mesh_dlo)
            .mesh_n_high(self.gossip_mesh_dhi)
            .gossip_lazy(self.gossip_mesh_dlazy)
            .flood_publish(self.gossip_flood_publish)
            .build()?;
        let block_time = config.block_time;

        let monitor_peers = self.ban_enabled.then_some(PeerMonitoring {
            ban_duration: Duration::from_secs(60 * self.ban_duration),
            ban_threshold: self.ban_threshold as f64,
        });

        let discovery_listening_address = SocketAddr::new(self.listen_ip, self.listen_udp_port);
        let discovery_config = self.discv5_config(discovery_listening_address.into(), static_ip);

        let mut gossip_address = libp2p::Multiaddr::from(self.listen_ip);
        gossip_address.push(libp2p::multiaddr::Protocol::Tcp(self.listen_tcp_port));

        Ok(Config {
            discovery_config,
            discovery_interval: Duration::from_secs(self.discovery_interval),
            discovery_address,
            discovery_randomize: self.discovery_randomize.map(Duration::from_secs),
            gossip_address,
            keypair,
            unsafe_block_signer: self.unsafe_block_signer(config, args, l1_rpc).await?,
            gossip_config,
            scoring: self.scoring,
            block_time,
            monitor_peers,
            bootstore: self.bootstore,
            topic_scoring: self.topic_scoring,
            redial: self.peer_redial,
            bootnodes: self.bootnodes,
            rollup_config: config.clone(),
        })
    }

    /// Returns the [Keypair] from the cli inputs.
    ///
    /// If the raw private key is empty and the specified file is empty,
    /// this method will generate a new private key and write it out to the file.
    ///
    /// If neither a file is specified, nor a raw private key input, this method
    /// will error.
    pub fn keypair(&self) -> Result<Keypair> {
        // Attempt the parse the private key if specified.
        if let Some(mut private_key) = self.private_key {
            return kona_cli::SecretKeyLoader::parse(&mut private_key.0)
                .map_err(|e| anyhow::anyhow!(e));
        }

        let Some(ref key_path) = self.priv_path else {
            anyhow::bail!("Neither a raw private key nor a private key file path was provided.");
        };

        kona_cli::SecretKeyLoader::load(key_path).map_err(|e| anyhow::anyhow!(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::b256;
    use clap::Parser;

    /// A mock command that uses the P2PArgs.
    #[derive(Parser, Debug, Clone)]
    #[command(about = "Mock command")]
    struct MockCommand {
        /// P2P CLI Flags
        #[clap(flatten)]
        pub p2p: P2PArgs,
    }

    #[test]
    fn test_p2p_args_keypair_missing_both() {
        let args = MockCommand::parse_from(["test"]);
        assert!(args.p2p.keypair().is_err());
    }

    #[test]
    fn test_p2p_args_keypair_raw_private_key() {
        let args = MockCommand::parse_from([
            "test",
            "--p2p.priv.raw",
            "1d2b0bda21d56b8bd12d4f94ebacffdfb35f5e226f84b461103bb8beab6353be",
        ]);
        assert!(args.p2p.keypair().is_ok());
    }

    #[test]
    fn test_p2p_args_keypair_from_path() {
        // Create a temporary directory.
        let dir = std::env::temp_dir();
        let mut source_path = dir.clone();
        assert!(std::env::set_current_dir(dir).is_ok());

        // Write a private key to a file.
        let key = b256!("1d2b0bda21d56b8bd12d4f94ebacffdfb35f5e226f84b461103bb8beab6353be");
        let hex = alloy_primitives::hex::encode(key.0);
        source_path.push("test.txt");
        std::fs::write(&source_path, &hex).unwrap();

        // Parse the keypair from the file.
        let args =
            MockCommand::parse_from(["test", "--p2p.priv.path", source_path.to_str().unwrap()]);
        assert!(args.p2p.keypair().is_ok());
    }

    #[test]
    fn test_p2p_args() {
        let args = MockCommand::parse_from(["test"]);
        assert_eq!(args.p2p, P2PArgs::default());
    }

    #[test]
    fn test_p2p_args_randomized() {
        let args = MockCommand::parse_from(["test", "--p2p.discovery.randomize", "10"]);
        assert_eq!(args.p2p.discovery_randomize, Some(10));
        let args = MockCommand::parse_from(["test"]);
        assert_eq!(args.p2p.discovery_randomize, None);
    }

    #[test]
    fn test_p2p_args_disabled() {
        let args = MockCommand::parse_from(["test", "--p2p.disable"]);
        assert!(args.p2p.disabled);
    }

    #[test]
    fn test_p2p_args_no_discovery() {
        let args = MockCommand::parse_from(["test", "--p2p.no-discovery"]);
        assert!(args.p2p.no_discovery);
    }

    #[test]
    fn test_p2p_args_priv_path() {
        let args = MockCommand::parse_from(["test", "--p2p.priv.path", "test.txt"]);
        assert_eq!(args.p2p.priv_path, Some(PathBuf::from("test.txt")));
    }

    #[test]
    fn test_p2p_args_private_key() {
        let args = MockCommand::parse_from([
            "test",
            "--p2p.priv.raw",
            "1d2b0bda21d56b8bd12d4f94ebacffdfb35f5e226f84b461103bb8beab6353be",
        ]);
        let key = b256!("1d2b0bda21d56b8bd12d4f94ebacffdfb35f5e226f84b461103bb8beab6353be");
        assert_eq!(args.p2p.private_key, Some(key));
    }

    #[test]
    fn test_p2p_args_listen_ip() {
        let args = MockCommand::parse_from(["test", "--p2p.listen.ip", "127.0.0.1"]);
        let expected: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(args.p2p.listen_ip, expected);
    }

    #[test]
    fn test_p2p_args_listen_tcp_port() {
        let args = MockCommand::parse_from(["test", "--p2p.listen.tcp", "1234"]);
        assert_eq!(args.p2p.listen_tcp_port, 1234);
    }

    #[test]
    fn test_p2p_args_listen_udp_port() {
        let args = MockCommand::parse_from(["test", "--p2p.listen.udp", "1234"]);
        assert_eq!(args.p2p.listen_udp_port, 1234);
    }
}
