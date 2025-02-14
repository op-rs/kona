#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod params;
pub use params::{
    base_fee_params, base_fee_params_canyon, BaseFeeConfig, BASE_SEPOLIA_BASE_FEE_CONFIG,
    BASE_SEPOLIA_BASE_FEE_PARAMS, BASE_SEPOLIA_BASE_FEE_PARAMS_CANYON,
    BASE_SEPOLIA_EIP1559_DEFAULT_ELASTICITY_MULTIPLIER, OP_MAINNET_BASE_FEE_CONFIG,
    OP_MAINNET_BASE_FEE_PARAMS, OP_MAINNET_BASE_FEE_PARAMS_CANYON,
    OP_MAINNET_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR_CANYON,
    OP_MAINNET_EIP1559_DEFAULT_BASE_FEE_MAX_CHANGE_DENOMINATOR,
    OP_MAINNET_EIP1559_DEFAULT_ELASTICITY_MULTIPLIER, OP_SEPOLIA_BASE_FEE_CONFIG,
    OP_SEPOLIA_BASE_FEE_PARAMS, OP_SEPOLIA_BASE_FEE_PARAMS_CANYON,
    OP_SEPOLIA_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR_CANYON,
    OP_SEPOLIA_EIP1559_DEFAULT_BASE_FEE_MAX_CHANGE_DENOMINATOR,
    OP_SEPOLIA_EIP1559_DEFAULT_ELASTICITY_MULTIPLIER,
};

mod superchain;
pub use superchain::{
    Superchain, SuperchainConfig, SuperchainL1Info, SuperchainLevel, Superchains,
};

mod updates;
pub use updates::{
    BatcherUpdate, Eip1559Update, GasConfigUpdate, GasLimitUpdate, OperatorFeeUpdate,
};

mod system;
pub use system::{
    BatcherUpdateError, EIP1559UpdateError, GasConfigUpdateError, GasLimitUpdateError,
    LogProcessingError, OperatorFeeUpdateError, SystemConfig, SystemConfigLog, SystemConfigUpdate,
    SystemConfigUpdateError, SystemConfigUpdateKind, CONFIG_UPDATE_EVENT_VERSION_0,
    CONFIG_UPDATE_TOPIC,
};

mod chain;
pub use chain::{
    AddressList, AltDAConfig, ChainConfig, HardForkConfiguration, Roles, BASE_MAINNET_CHAIN_ID,
    BASE_SEPOLIA_CHAIN_ID, OP_MAINNET_CHAIN_ID, OP_SEPOLIA_CHAIN_ID,
};

mod genesis;
pub use genesis::ChainGenesis;

mod rollup;
pub use rollup::{
    RollupConfig, DEFAULT_INTEROP_MESSAGE_EXPIRY_WINDOW, FJORD_MAX_SEQUENCER_DRIFT,
    GRANITE_CHANNEL_TIMEOUT, MAX_RLP_BYTES_PER_CHANNEL_BEDROCK, MAX_RLP_BYTES_PER_CHANNEL_FJORD,
};
