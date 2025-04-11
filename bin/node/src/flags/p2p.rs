//! P2P CLI Flags
//!
//! These are based on p2p flags from the [`op-node`][op-node] CLI.
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/flags/p2p_flags.go

use crate::flags::GlobalArgs;
use alloy_primitives::B256;
use anyhow::Result;
use clap::Parser;
use kona_p2p::{Config, PeerMonitoring, PeerScoreLevel};
use libp2p::identity::Keypair;
use std::{
    net::{IpAddr, SocketAddr},
    num::ParseIntError,
    path::PathBuf,
};
use tokio::time::Duration;

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
    /// IP to bind LibP2P and Discv5 to.
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
    /// Configure GossipSub topic stablel mesh target count.
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
    #[arg(long = "p2p.scoring", default_value = "light", env = "KONA_NODE_P2P_SCORING")]
    pub scoring: PeerScoreLevel,

    /// Allows to ban peers based on their score.
    ///
    /// Peers are banned based on a ban threshold (see `p2p.ban.threshold`).
    /// If a peer's score is below the threshold, it gets automatically banned.
    #[arg(long = "p2p.ban.peers", default_value = "false", env = "KONA_NODE_P2P_BAN_PEERS")]
    pub ban_enabled: bool,

    /// The threshold used to ban peers.
    /// Note that for peers to be banned, the `p2p.ban.peers` flag must be set to `true`.
    #[arg(long = "p2p.ban.threshold", default_value = "0", env = "KONA_NODE_P2P_BAN_THRESHOLD")]
    pub ban_threshold: i32,

    /// The duration in seconds to ban a peer for.
    /// Note that for peers to be banned, the `p2p.ban.peers` flag must be set to `true`.
    #[arg(long = "p2p.ban.duration", default_value = "30", env = "KONA_NODE_P2P_BAN_DURATION")]
    pub ban_duration: u32,
}

impl Default for P2PArgs {
    fn default() -> Self {
        Self {
            disabled: false,
            no_discovery: false,
            priv_path: None,
            private_key: None,
            listen_ip: "0.0.0.0".parse().unwrap(),
            listen_tcp_port: 9222,
            listen_udp_port: 9223,
            peers_lo: 20,
            peers_hi: 30,
            peers_grace: Duration::from_secs(30),
            gossip_mesh_d: kona_p2p::DEFAULT_MESH_D,
            gossip_mesh_dlo: kona_p2p::DEFAULT_MESH_DLO,
            gossip_mesh_dhi: kona_p2p::DEFAULT_MESH_DHI,
            gossip_mesh_dlazy: kona_p2p::DEFAULT_MESH_DLAZY,
            gossip_flood_publish: false,
            scoring: PeerScoreLevel::Light,
            ban_enabled: false,
            ban_threshold: 0,
            ban_duration: 30,
        }
    }
}

impl P2PArgs {
    /// Constructs kona's P2P network [`Config`] from CLI arguments.
    ///
    /// ## Parameters
    ///
    /// - [`GlobalArgs`]: required to fetch the genesis unsafe block signer.
    ///
    /// Errors if the genesis unsafe block signer isn't available for the specified L2 Chain ID.
    pub fn config(&self, args: &GlobalArgs) -> anyhow::Result<Config> {
        let mut multiaddr = libp2p::Multiaddr::from(self.listen_ip);
        multiaddr.push(libp2p::multiaddr::Protocol::Tcp(self.listen_tcp_port));
        let gossip_config = kona_p2p::default_config_builder()
            .mesh_n(self.gossip_mesh_d)
            .mesh_n_low(self.gossip_mesh_dlo)
            .mesh_n_high(self.gossip_mesh_dhi)
            .gossip_lazy(self.gossip_mesh_dlazy)
            .flood_publish(self.gossip_flood_publish)
            .build()?;
        let block_time = args.block_time()?;

        let monitor_peers = if self.ban_enabled {
            Some(PeerMonitoring {
                ban_duration: Duration::from_secs(self.ban_duration.into()),
                ban_threshold: self.ban_threshold as f64,
            })
        } else {
            None
        };

        Ok(Config {
            discovery_address: SocketAddr::new(self.listen_ip, self.listen_udp_port),
            gossip_address: multiaddr,
            keypair: self.keypair().unwrap_or_else(|_| Keypair::generate_secp256k1()),
            unsafe_block_signer: args.genesis_signer()?,
            gossip_config,
            scoring: self.scoring,
            block_time,
            monitor_peers,
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
            return kona_p2p::parse_key(&mut private_key.0).map_err(|e| anyhow::anyhow!(e));
        }

        let Some(ref key_path) = self.priv_path else {
            anyhow::bail!("Neither a raw private key nor a private key file path was provided.");
        };

        kona_p2p::get_keypair(key_path).map_err(|e| anyhow::anyhow!(e))
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
    pub struct MockCommand {
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
