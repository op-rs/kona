// Reproduces panic: "failed to create rollup boost server: Missing Client JWT secret" when
// constructing rollup-boost without an L2 client JWT provided.

#[cfg(test)]
mod tests {
    use http::Uri;
    use rollup_boost::{FlashblocksArgs, Probes, RollupBoostArgs, RollupBoostServer};
    use std::sync::Arc;

    #[test]
    fn repro_missing_client_jwt_secret() {
        // Build args with execution enabled and flashblocks enabled but NO L2 JWT provided.
        // This mirrors the failing acceptance configuration when no client JWT is wired through.
        let args = RollupBoostArgs {
            builder: rollup_boost::BuilderArgs {
                // Any URI; builder may be disabled at runtime, but server creation still validates
                // args.
                builder_url: "http://127.0.0.1:8551".parse::<Uri>().unwrap(),
                builder_jwt_token: None,
                builder_jwt_path: None, // intentionally missing
                builder_timeout: 1000,
            },
            l2_client: rollup_boost::L2ClientArgs {
                l2_url: "http://127.0.0.1:8551".parse::<Uri>().unwrap(),
                l2_jwt_token: None,
                l2_jwt_path: None, /* intentionally missing -> triggers the panic/error in server
                                    * ctor */
                l2_timeout: 1000,
            },
            // Default is ExecutionMode::Enabled in the crate; rely on that or set explicitly if
            // needed.
            flashblocks: FlashblocksArgs {
                flashblocks: true,
                flashblocks_builder_url: "ws://127.0.0.1:1111".parse().unwrap(),
                flashblocks_host: "127.0.0.1".to_string(),
                flashblocks_port: 1112,
                flashblock_builder_ws_reconnect_ms: 5000,
            },
            ..Default::default()
        };

        let probes = Arc::new(Probes::default());
        let err =
            RollupBoostServer::<rollup_boost::FlashblocksService>::new_from_args(args, probes)
                .expect_err("expected missing JWT to error");
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("missing client jwt secret"),
            "unexpected error: {msg}"
        );
    }
}
