use std::path::PathBuf;

use alloy_rpc_types_engine::JwtSecret;
use rollup_boost::RollupBoostLibArgs;

/// Custom block builder flags.
#[derive(Clone, Debug, clap::Subcommand)]
pub enum CustomBlockBuilder {
    /// Rollup Boost block builder.
    RollupBoost(RollupBoostLibArgs),
}

impl CustomBlockBuilder {
    /// Returns the L2 JWT token for the custom block builder.
    pub const fn l2_jwt_token(&self) -> Option<JwtSecret> {
        match self {
            Self::RollupBoost(args) => args.l2_client.l2_jwt_token,
        }
    }

    /// Returns the L2 JWT path for the custom block builder.
    pub fn l2_jwt_path(&self) -> Option<PathBuf> {
        match self {
            Self::RollupBoost(args) => args.l2_client.l2_jwt_path.clone(),
        }
    }

    /// Returns the builder JWT path for the custom block builder.
    pub fn builder_jwt_path(&self) -> Option<PathBuf> {
        match self {
            Self::RollupBoost(args) => args.builder.builder_jwt_path.clone(),
        }
    }

    /// Returns the rollup boost args for the custom block builder.
    pub const fn as_rollup_boost_args(&self) -> Option<&RollupBoostLibArgs> {
        match self {
            Self::RollupBoost(args) => Some(args),
        }
    }
}
