//! P2P CLI Flags
//!
//! These are based on p2p flags from the [`op-node`][op-node] CLI.
//!
//! [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/flags/p2p_flags.go

use alloy_primitives::B256;
use clap::Parser;
use std::{net::IpAddr, path::PathBuf};

/// P2P CLI Flags
#[derive(Parser, Clone, Debug, PartialEq, Eq)]
pub struct P2PArgs {
    /// Whether to disable the entire P2P stack.
    #[clap(
        long = "p2p.disable",
        default_value = "false",
        env = "KONA_NODE_P2P_DISABLE",
        help = "Completely disable the P2P stack"
    )]
    pub disabled: bool,
    /// Disable Discv5 (node discovery).
    #[clap(
        long = "p2p.no-discovery",
        default_value = "false",
        env = "KONA_NODE_P2P_NO_DISCOVERY",
        help = "Disable Discv5 (node discovery)"
    )]
    pub no_discovery: bool,
    /// A private key file path.
    #[clap(
        long = "p2p.priv.path",
        env = "KONA_NODE_P2P_PRIV_PATH",
        help = "Read the hex-encoded 32-byte private key for the peer ID from this txt file. Created if not already exists. Important to persist to keep the same network identity after restarting, maintaining the previous advertised identity."
    )]
    pub priv_path: Option<PathBuf>,
    /// The hex-encoded 32-byte private key for the peer ID.
    #[clap(
        long = "p2p.priv.raw",
        env = "KONA_NODE_P2P_PRIV_RAW",
        help = "The hex-encoded 32-byte private key for the peer ID"
    )]
    pub private_key: Option<B256>,
    /// IP to bind LibP2P and Discv5 to.
    #[clap(
        long = "p2p.listen.ip",
        default_value = "0.0.0.0",
        env = "KONA_NODE_P2P_LISTEN_IP",
        help = "IP to bind LibP2P and Discv5 to"
    )]
    pub listen_ip: IpAddr,
    /// TCP port to bind LibP2P to. Any available system port if set to 0.
    #[clap(
        long = "p2p.listen.tcp",
        default_value = "9222",
        env = "KONA_NODE_P2P_LISTEN_TCP_PORT",
        help = "TCP port to bind LibP2P to. Any available system port if set to 0."
    )]
    pub listen_tcp_port: u16,
    /// UDP port to bind Discv5 to. Same as TCP port if left 0.
    #[clap(
        long = "p2p.listen.udp",
        default_value = "0",
        env = "KONA_NODE_P2P_LISTEN_UDP_PORT",
        help = "UDP port to bind Discv5 to. Same as TCP port if left 0."
    )]
    pub listen_udp_port: u16,
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
            listen_udp_port: 0,
        }
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
