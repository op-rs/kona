//! Integration tests for rollup-boost client.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::{RollupBoostArgs, RollupBoostExecutionMode, RollupBoostBlockSelectionPolicy};
    use alloy_rpc_types_engine::JwtSecret;
    use kona_genesis::RollupConfig;
    use std::{path::PathBuf, sync::Arc};
    use url::Url;

    #[tokio::test]
    async fn test_rollup_boost_client_disabled_mode() {
        let args = RollupBoostArgs::default();
        let l2_engine_url = Url::parse("http://localhost:8551").unwrap();
        let l1_rpc_url = Url::parse("http://localhost:8545").unwrap();
        let jwt_secret = JwtSecret::random();
        let rollup_config = Arc::new(RollupConfig::default());

        let client = RollupBoostEngineClient::new(
            args,
            l2_engine_url,
            l1_rpc_url,
            jwt_secret,
            rollup_config,
        ).await.unwrap();

        // Should not have rollup-boost instance when disabled
        assert!(client.rollup_boost.is_none());
        assert_eq!(client.health().await, rollup_boost::Health::Healthy);
        assert_eq!(client.execution_mode().await, RollupBoostExecutionMode::Enabled);
    }

    #[tokio::test]
    async fn test_rollup_boost_client_enabled_mode_no_builder() {
        let mut args = RollupBoostArgs::default();
        args.enabled = true;
        args.builder_jwt_secret_file = Some(std::path::PathBuf::from("test_jwt.hex"));

        let l2_engine_url = Url::parse("http://localhost:8551").unwrap();
        let l1_rpc_url = Url::parse("http://localhost:8545").unwrap();
        let jwt_secret = JwtSecret::random();
        let rollup_config = Arc::new(RollupConfig::default());

        // This test validates graceful error handling when builder is unavailable
        let result = RollupBoostEngineClient::new(
            args,
            l2_engine_url,
            l1_rpc_url,
            jwt_secret,
            rollup_config,
        ).await;

        // Should either succeed (using fallback) or fail with a connection error
        match result {
            Ok(_) => {
                // Success means fallback to standard client worked
            }
            Err(e) => {
                // Error should be connection-related, not configuration error
                let err_str = e.to_string();
                assert!(
                    err_str.contains("connect") 
                        || err_str.contains("unavailable")
                        || err_str.contains("refused")
                        || err_str.contains("timeout"),
                    "Expected connection-related error, got: {}", err_str
                );
            }
        }
    }


    #[test]
    fn test_rollup_boost_config_validation() {
        let mut args = RollupBoostArgs::default();

        // Disabled should pass validation
        assert!(args.validate().is_ok());

        // Enabled without builder URL should fail
        args.enabled = true;
        args.builder_jwt_secret_file = Some(std::path::PathBuf::from("jwt.hex"));
        assert!(args.validate().is_err());

        // Enabled without JWT secret should fail
        args.builder_url = Some(http::Uri::from_static("http://localhost:8551"));
        args.builder_jwt_secret_file = None;
        assert!(args.validate().is_err());

        // Enabled with both should pass
        args.builder_jwt_secret_file = Some(std::path::PathBuf::from("jwt.hex"));
        assert!(args.validate().is_ok());
    }

    #[test]
    fn test_execution_mode_conversions() {
        use rollup_boost::ExecutionMode;

        // Test conversions between our enum and rollup-boost's enum
        assert_eq!(ExecutionMode::from(RollupBoostExecutionMode::Enabled), ExecutionMode::Enabled);
        assert_eq!(ExecutionMode::from(RollupBoostExecutionMode::DryRun), ExecutionMode::DryRun);
        assert_eq!(ExecutionMode::from(RollupBoostExecutionMode::Disabled), ExecutionMode::Disabled);
    }

    #[test]
    fn test_block_selection_policy_conversions() {
        use rollup_boost::BlockSelectionPolicy;

        // Test conversions between our enum and rollup-boost's enum
        assert_eq!(<Option<BlockSelectionPolicy>>::from(RollupBoostBlockSelectionPolicy::BuilderPreference), None);
        assert_eq!(<Option<BlockSelectionPolicy>>::from(RollupBoostBlockSelectionPolicy::GasUsed), Some(BlockSelectionPolicy::GasUsed));
    }

    #[tokio::test]
    async fn test_health_status_updates() {
        let args = RollupBoostArgs::default();
        let l2_engine_url = Url::parse("http://localhost:8551").unwrap();
        let l1_rpc_url = Url::parse("http://localhost:8545").unwrap();
        let jwt_secret = JwtSecret::random();
        let rollup_config = Arc::new(RollupConfig::default());

        let client = RollupBoostEngineClient::new(
            args,
            l2_engine_url,
            l1_rpc_url,
            jwt_secret,
            rollup_config,
        ).await.unwrap();

        // Should start with Healthy status when rollup-boost is disabled
        assert_eq!(client.health().await, rollup_boost::Health::Healthy);

        // Update health status (should remain Healthy since rollup-boost is disabled)
        client.update_health_status().await;
        assert_eq!(client.health().await, rollup_boost::Health::Healthy);
    }

    #[tokio::test]
    async fn test_mode_switching() {
        let args = RollupBoostArgs::default();
        let l2_engine_url = Url::parse("http://localhost:8551").unwrap();
        let l1_rpc_url = Url::parse("http://localhost:8545").unwrap();
        let jwt_secret = JwtSecret::random();
        let rollup_config = Arc::new(RollupConfig::default());

        let client = RollupBoostEngineClient::new(
            args,
            l2_engine_url,
            l1_rpc_url,
            jwt_secret,
            rollup_config,
        ).await.unwrap();

        // Should start with default execution mode
        assert_eq!(client.execution_mode().await, RollupBoostExecutionMode::Enabled);

        // Mode switching should work even when rollup-boost is disabled
        client.set_execution_mode(RollupBoostExecutionMode::Disabled).await.unwrap();
        assert_eq!(client.execution_mode().await, RollupBoostExecutionMode::Disabled);

        client.set_execution_mode(RollupBoostExecutionMode::DryRun).await.unwrap();
        assert_eq!(client.execution_mode().await, RollupBoostExecutionMode::DryRun);
    }

    #[tokio::test]
    async fn test_auto_mode_switching_disabled_rollup_boost() {
        let args = RollupBoostArgs::default();
        let l2_engine_url = Url::parse("http://localhost:8551").unwrap();
        let l1_rpc_url = Url::parse("http://localhost:8545").unwrap();
        let jwt_secret = JwtSecret::random();
        let rollup_config = Arc::new(RollupConfig::default());

        let client = RollupBoostEngineClient::new(
            args,
            l2_engine_url,
            l1_rpc_url,
            jwt_secret,
            rollup_config,
        ).await.unwrap();

        // Auto-switching should work even when rollup-boost is disabled
        // (it should just return Ok without doing anything)
        client.auto_switch_mode_based_on_health().await.unwrap();
    }

}
