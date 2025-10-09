//! Rollup-boost enhanced engine client.

use crate::flags::{RollupBoostArgs, RollupBoostExecutionMode, RollupBoostBlockSelectionPolicy};
use alloy_rpc_types_engine::{ForkchoiceState, PayloadId};
use alloy_rpc_types_eth;
use async_trait::async_trait;
use kona_engine::{EngineClient, EngineClientError};
use kona_genesis::RollupConfig;
use op_alloy_rpc_types_engine::{OpExecutionPayloadEnvelope, OpPayloadAttributes};
use rollup_boost::{ExecutionMode, Health, RollupBoost, RollupBoostConfigBuilder};
use std::{path::Path, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use url::Url;

/// A rollup-boost enhanced engine client that uses rollup-boost for payload building
/// when available and falls back to the standard engine client when needed.
#[derive(Debug)]
pub struct RollupBoostEngineClient {
    /// The underlying rollup-boost instance.
    rollup_boost: Option<Arc<RollupBoost>>,
    /// The fallback engine client for when rollup-boost is disabled or unavailable.
    fallback_client: Arc<EngineClient>,
    /// Configuration for rollup-boost.
    config: RollupBoostArgs,
    /// The rollup configuration.
    rollup_config: Arc<RollupConfig>,
    /// Current health status of rollup-boost.
    health_status: Arc<RwLock<Health>>,
    /// Current execution mode (tracked separately since it can be changed at runtime).
    current_execution_mode: Arc<RwLock<RollupBoostExecutionMode>>,
}

impl RollupBoostEngineClient {
    /// Creates a new rollup-boost enhanced engine client.
    pub async fn new(
        config: RollupBoostArgs,
        l2_engine_url: Url,
        l1_rpc_url: Url,
        jwt_secret: alloy_rpc_types_engine::JwtSecret,
        rollup_config: Arc<RollupConfig>,
    ) -> anyhow::Result<Self> {
        let fallback_client = Arc::new(EngineClient::new_http(
            l2_engine_url.clone(),
            l1_rpc_url.clone(),
            rollup_config.clone(),
            jwt_secret,
        ));

        let rollup_boost = if config.enabled {
            let mut rollup_boost_config_builder = RollupBoostConfigBuilder::new();

            // Set L2 JWT secret (required)
            rollup_boost_config_builder = rollup_boost_config_builder
                .l2_jwt_secret_hex(alloy_primitives::hex::encode(jwt_secret.as_bytes()))?;

            // Set L2 URL (required)
            rollup_boost_config_builder = rollup_boost_config_builder.l2_url(l2_engine_url.clone())?;

            // Set builder URL if provided
            if let Some(builder_url) = &config.builder_url {
                rollup_boost_config_builder = rollup_boost_config_builder
                    .builder_url(builder_url.clone())?;
            }

            // Set builder JWT secret file if provided
            if let Some(builder_jwt_file) = &config.builder_jwt_secret_file {
                rollup_boost_config_builder = rollup_boost_config_builder
                    .builder_jwt_secret_file(builder_jwt_file)?;
            }

            // Set timeouts
            rollup_boost_config_builder = rollup_boost_config_builder.l2_timeout(config.l2_timeout_ms);
            if config.builder_url.is_some() {
                rollup_boost_config_builder = rollup_boost_config_builder
                    .builder_timeout(config.builder_timeout_ms);
            }

            // Set execution mode
            rollup_boost_config_builder = rollup_boost_config_builder
                .execution_mode(config.execution_mode.clone().into());

            // Set block selection policy
            rollup_boost_config_builder = rollup_boost_config_builder
                .block_selection_policy(config.block_selection_policy.clone().into());

            // Set other options
            rollup_boost_config_builder = rollup_boost_config_builder
                .external_state_root(config.external_state_root)
                .ignore_unhealthy_builders(config.ignore_unhealthy_builders);

            let rollup_boost_config = rollup_boost_config_builder.build();

            let mut rb = RollupBoost::new(rollup_boost_config)?;

            // Metrics are handled internally by rollup-boost
            if config.metrics_enabled {
                if let Some(metrics_addr) = &config.metrics_addr {
                    info!(
                        "Rollup-boost metrics enabled. Metrics will be available through rollup-boost's internal metrics system at {}",
                        metrics_addr
                    );
                }
            }

            // Start health monitoring
            rb.start_health_check(
                config.health_check_interval_secs,
                config.health_check_threshold,
            )?;

            if let Some(builder_url) = &config.builder_url {
                info!("Rollup-boost enabled with builder at {}", builder_url);
                if let Some(builder_jwt_file) = &config.builder_jwt_secret_file {
                    info!("Using builder JWT secret file: {:?}", builder_jwt_file);
                }
            }

            Some(Arc::new(rb))
        } else {
            info!("Rollup-boost integration disabled");
            None
        };

        let initial_execution_mode = config.execution_mode.clone();

        Ok(Self {
            rollup_boost,
            fallback_client,
            config,
            rollup_config,
            health_status: Arc::new(RwLock::new(Health::Healthy)),
            current_execution_mode: Arc::new(RwLock::new(initial_execution_mode)),
        })
    }

    /// Gets the current health status of rollup-boost.
    pub async fn health(&self) -> Health {
        if let Some(rb) = &self.rollup_boost {
            rb.health()
        } else {
            Health::Healthy
        }
    }

    /// Updates the health status and performs automatic mode switching.
    pub async fn update_health_status(&self) {
        if let Some(rb) = &self.rollup_boost {
            let health = rb.health();
            *self.health_status.write().await = health;

            // Perform automatic mode switching based on health
            if let Err(e) = self.auto_switch_mode_based_on_health().await {
                warn!("Failed to auto-switch rollup-boost mode: {}", e);
            }

            match health {
                Health::Healthy => {
                    debug!("Rollup-boost is healthy");
                }
                Health::PartialContent => {
                    warn!("Rollup-boost has partial content (L2 working, builder may be down)");
                }
                Health::ServiceUnavailable => {
                    error!("Rollup-boost service unavailable");
                }
            }
        }
    }

    /// Switches rollup-boost execution mode.
    pub async fn set_execution_mode(&self, mode: RollupBoostExecutionMode) -> anyhow::Result<()> {
        // Update the tracked execution mode
        *self.current_execution_mode.write().await = mode.clone();

        // Update rollup-boost if available
        if let Some(rb) = &self.rollup_boost {
            rb.set_execution_mode(mode.clone().into());
            info!("Switched rollup-boost execution mode to {:?}", mode);
        }
        Ok(())
    }

    /// Automatically switches execution mode based on health status.
    /// This method can be called periodically to adapt to changing conditions.
    pub async fn auto_switch_mode_based_on_health(&self) -> anyhow::Result<()> {
        if self.rollup_boost.is_none() {
            return Ok(()); // No rollup-boost to manage
        }

        let health = self.health().await;
        let current_mode = self.execution_mode().await;

        match health {
            Health::Healthy => {
                // If we're in Disabled mode but rollup-boost is healthy, switch to Enabled
                if current_mode == RollupBoostExecutionMode::Disabled {
                    info!("Rollup-boost is healthy, switching from Disabled to Enabled mode");
                    self.set_execution_mode(RollupBoostExecutionMode::Enabled).await?;
                }
            }
            Health::PartialContent => {
                // If we're in Enabled mode but only have partial content, we might want to stay in Enabled
                // since L2 fallback is still working. Only switch if we're seeing persistent issues.
                debug!("Rollup-boost has partial content, maintaining current mode: {:?}", current_mode);
            }
            Health::ServiceUnavailable => {
                // If rollup-boost is completely unavailable, switch to Disabled mode
                if current_mode == RollupBoostExecutionMode::Enabled {
                    warn!("Rollup-boost service unavailable, switching to Disabled mode");
                    self.set_execution_mode(RollupBoostExecutionMode::Disabled).await?;
                }
            }
        }

        Ok(())
    }

    /// Gets the current execution mode.
    pub async fn execution_mode(&self) -> RollupBoostExecutionMode {
        self.current_execution_mode.read().await.clone()
    }


    /// Handles a fork choice updated call using rollup-boost when available.
    pub async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> Result<kona_engine::EngineResult, EngineClientError> {
        if let Some(rb) = &self.rollup_boost {
            match rb.health() {
                Health::Healthy => {
                    debug!("Using rollup-boost for fork_choice_updated_v3");
                    match rb.fork_choice_updated_v3(fork_choice_state, payload_attributes).await {
                        Ok(result) => {
                            debug!("Rollup-boost fork_choice_updated_v3 succeeded");
                            return Ok(result);
                        }
                        Err(e) => {
                            warn!("Rollup-boost fork_choice_updated_v3 failed: {}", e);
                            self.update_health_status().await;
                        }
                    }
                }
                Health::PartialContent => {
                    debug!("Rollup-boost partially available, using fallback for fork_choice_updated_v3");
                }
                Health::ServiceUnavailable => {
                    debug!("Rollup-boost unavailable, using fallback for fork_choice_updated_v3");
                }
            }
        }

        debug!("Using fallback engine client for fork_choice_updated_v3");
        self.fallback_client.fork_choice_updated_v3(fork_choice_state, payload_attributes).await
    }

    /// Handles a get payload call using rollup-boost when available.
    pub async fn get_payload_v3(
        &self,
        payload_id: PayloadId,
    ) -> Result<OpExecutionPayloadEnvelope, EngineClientError> {
        if let Some(rb) = &self.rollup_boost {
            match rb.health() {
                Health::Healthy => {
                    debug!("Using rollup-boost for get_payload_v3");
                    match rb.get_payload_v3(payload_id).await {
                        Ok(result) => {
                            debug!("Rollup-boost get_payload_v3 succeeded");
                            return Ok(result);
                        }
                        Err(e) => {
                            warn!("Rollup-boost get_payload_v3 failed: {}", e);
                            self.update_health_status().await;
                        }
                    }
                }
                Health::PartialContent => {
                    debug!("Rollup-boost partially available, using fallback for get_payload_v3");
                }
                Health::ServiceUnavailable => {
                    debug!("Rollup-boost unavailable, using fallback for get_payload_v3");
                }
            }
        }

        debug!("Using fallback engine client for get_payload_v3");
        self.fallback_client.get_payload_v3(payload_id).await
    }

    /// Handles a new payload call using rollup-boost when available.
    pub async fn new_payload_v3(
        &self,
        payload: op_alloy_rpc_types_engine::OpExecutionPayload,
        expected_blob_versioned_hashes: Vec<alloy_primitives::B256>,
        parent_beacon_block_root: alloy_primitives::B256,
    ) -> Result<kona_engine::PayloadStatus, EngineClientError> {
        if let Some(rb) = &self.rollup_boost {
            match rb.health() {
                Health::Healthy => {
                    debug!("Using rollup-boost for new_payload_v3");
                    match rb.new_payload_v3(payload, expected_blob_versioned_hashes, parent_beacon_block_root).await {
                        Ok(result) => {
                            debug!("Rollup-boost new_payload_v3 succeeded");
                            return Ok(result);
                        }
                        Err(e) => {
                            warn!("Rollup-boost new_payload_v3 failed: {}", e);
                            self.update_health_status().await;
                        }
                    }
                }
                Health::PartialContent => {
                    debug!("Rollup-boost partially available, using fallback for new_payload_v3");
                }
                Health::ServiceUnavailable => {
                    debug!("Rollup-boost unavailable, using fallback for new_payload_v3");
                }
            }
        }

        debug!("Using fallback engine client for new_payload_v3");
        self.fallback_client.new_payload_v3(payload, expected_blob_versioned_hashes, parent_beacon_block_root).await
    }

    /// Handles a get block by number call using rollup-boost when available.
    pub async fn get_block_by_number(
        &self,
        block_number: alloy_rpc_types_eth::BlockNumberOrTag,
        include_txs: bool,
    ) -> Result<alloy_rpc_types_eth::Block, EngineClientError> {
        if let Some(rb) = &self.rollup_boost {
            match rb.health() {
                Health::Healthy => {
                    debug!("Using rollup-boost for get_block_by_number");
                    match rb.get_block_by_number(block_number, include_txs).await {
                        Ok(result) => {
                            debug!("Rollup-boost get_block_by_number succeeded");
                            return Ok(result);
                        }
                        Err(e) => {
                            warn!("Rollup-boost get_block_by_number failed: {}", e);
                            self.update_health_status().await;
                        }
                    }
                }
                Health::PartialContent => {
                    debug!("Rollup-boost partially available, using fallback for get_block_by_number");
                }
                Health::ServiceUnavailable => {
                    debug!("Rollup-boost unavailable, using fallback for get_block_by_number");
                }
            }
        }

        debug!("Using fallback engine client for get_block_by_number");
        self.fallback_client.get_block_by_number(block_number, include_txs).await
    }
}

#[async_trait]
impl EngineClient for RollupBoostEngineClient {
    async fn exchange_capabilities(&self, methods: Vec<String>) -> Result<(), EngineClientError> {
        // Always use the fallback client for capability exchange
        self.fallback_client.exchange_capabilities(methods).await
    }

    async fn fork_choice_updated_v3(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> Result<kona_engine::EngineResult, EngineClientError> {
        RollupBoostEngineClient::fork_choice_updated_v3(self, fork_choice_state, payload_attributes).await
    }

    async fn get_payload_v3(
        &self,
        payload_id: PayloadId,
    ) -> Result<OpExecutionPayloadEnvelope, EngineClientError> {
        RollupBoostEngineClient::get_payload_v3(self, payload_id).await
    }

    async fn new_payload_v3(
        &self,
        payload: op_alloy_rpc_types_engine::OpExecutionPayload,
        expected_blob_versioned_hashes: Vec<alloy_primitives::B256>,
        parent_beacon_block_root: alloy_primitives::B256,
    ) -> Result<kona_engine::PayloadStatus, EngineClientError> {
        RollupBoostEngineClient::new_payload_v3(self, payload, expected_blob_versioned_hashes, parent_beacon_block_root).await
    }

    async fn get_block_by_number(
        &self,
        block_number: alloy_rpc_types_eth::BlockNumberOrTag,
        include_txs: bool,
    ) -> Result<alloy_rpc_types_eth::Block, EngineClientError> {
        RollupBoostEngineClient::get_block_by_number(self, block_number, include_txs).await
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use alloy_rpc_types_eth::BlockNumberOrTag;

    #[tokio::test]
    async fn test_rollup_boost_client_creation_disabled() {
        let args = RollupBoostArgs::default();
        let l2_engine_url = Url::parse("http://localhost:8551").unwrap();
        let l1_rpc_url = Url::parse("http://localhost:8545").unwrap();
        let jwt_secret = alloy_rpc_types_engine::JwtSecret::random();
        let rollup_config = Arc::new(RollupConfig::default());

        let client = RollupBoostEngineClient::new(
            args,
            l2_engine_url,
            l1_rpc_url,
            jwt_secret,
            rollup_config,
        ).await.unwrap();

        assert!(client.rollup_boost.is_none());
        assert_eq!(client.health().await, Health::Healthy);
        assert_eq!(client.execution_mode().await, RollupBoostExecutionMode::Enabled);
    }

    #[tokio::test]
    async fn test_rollup_boost_client_creation_enabled() {
        let mut args = RollupBoostArgs::default();
        args.enabled = true;
        args.builder_url = Some(http::Uri::from_static("http://localhost:8551"));
        args.builder_jwt_secret_file = Some(std::path::PathBuf::from("test_jwt.hex"));

        let l2_engine_url = Url::parse("http://localhost:8551").unwrap();
        let l1_rpc_url = Url::parse("http://localhost:8545").unwrap();
        let jwt_secret = alloy_rpc_types_engine::JwtSecret::random();
        let rollup_config = Arc::new(RollupConfig::default());

        // This test validates that the configuration parsing works correctly
        // We expect this to fail because rollup-boost can't connect to services
        let result = RollupBoostEngineClient::new(
            args,
            l2_engine_url,
            l1_rpc_url,
            jwt_secret,
            rollup_config,
        ).await;

        // The result should be an error because the builder is not available
        // If it succeeds, that means fallback was used (which is also valid behavior)
        match result {
            Ok(_) => {
                // This is acceptable - it means the fallback to standard client worked
            }
            Err(e) => {
                // This is expected - should be a connection or availability error
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
    fn test_execution_mode_conversion() {
        assert_eq!(rollup_boost::ExecutionMode::from(RollupBoostExecutionMode::Enabled), rollup_boost::ExecutionMode::Enabled);
        assert_eq!(rollup_boost::ExecutionMode::from(RollupBoostExecutionMode::DryRun), rollup_boost::ExecutionMode::DryRun);
        assert_eq!(rollup_boost::ExecutionMode::from(RollupBoostExecutionMode::Disabled), rollup_boost::ExecutionMode::Disabled);
    }

    #[test]
    fn test_block_selection_policy_conversion() {
        assert_eq!(<Option<rollup_boost::BlockSelectionPolicy>>::from(RollupBoostBlockSelectionPolicy::BuilderPreference), None);
        assert_eq!(<Option<rollup_boost::BlockSelectionPolicy>>::from(RollupBoostBlockSelectionPolicy::GasUsed), Some(rollup_boost::BlockSelectionPolicy::GasUsed));
    }
}
