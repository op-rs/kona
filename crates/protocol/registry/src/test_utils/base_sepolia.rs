//! Base Sepolia Rollup Config.

use alloy_eips::BlockNumHash;
use alloy_primitives::{address, b256, uint};
use kona_genesis::{
    BASE_SEPOLIA_BASE_FEE_PARAMS, BASE_SEPOLIA_BASE_FEE_PARAMS_CANYON, ChainGenesis,
    DEFAULT_INTEROP_MESSAGE_EXPIRY_WINDOW, HardForkConfig, RollupConfig, SystemConfig,
};

/// The [RollupConfig] for Base Sepolia.
pub const BASE_SEPOLIA_CONFIG: RollupConfig = RollupConfig {
    genesis: ChainGenesis {
        l1: BlockNumHash {
            hash: b256!("cac9a83291d4dec146d6f7f69ab2304f23f5be87b1789119a0c5b1e4482444ed"),
            number: 4370868,
        },
        l2: BlockNumHash {
            hash: b256!("0dcc9e089e30b90ddfc55be9a37dd15bc551aeee999d2e2b51414c54eaf934e4"),
            number: 0,
        },
        l2_time: 1695768288,
        system_config: Some(SystemConfig {
            batcher_address: address!("6cdebe940bc0f26850285caca097c11c33103e47"),
            overhead: uint!(0x834_U256),
            scalar: uint!(0xf4240_U256),
            gas_limit: 25000000,
            base_fee_scalar: None,
            blob_base_fee_scalar: None,
            eip1559_denominator: None,
            eip1559_elasticity: None,
            operator_fee_scalar: None,
            operator_fee_constant: None,
        }),
    },
    block_time: 2,
    max_sequencer_drift: 600,
    seq_window_size: 3600,
    channel_timeout: 300,
    granite_channel_timeout: 50,
    l1_chain_id: 11155111,
    l2_chain_id: 84532,
    base_fee_params: BASE_SEPOLIA_BASE_FEE_PARAMS,
    canyon_base_fee_params: BASE_SEPOLIA_BASE_FEE_PARAMS_CANYON,
    hardforks: HardForkConfig {
        regolith_time: None,
        canyon_time: Some(1699981200),
        delta_time: Some(1703203200),
        ecotone_time: Some(1708534800),
        fjord_time: Some(1716998400),
        granite_time: Some(1723478400),
        holocene_time: Some(1732633200),
        isthmus_time: None,
        interop_time: None,
    },
    batch_inbox_address: address!("ff00000000000000000000000000000000084532"),
    deposit_contract_address: address!("49f53e41452c74589e85ca1677426ba426459e85"),
    l1_system_config_address: address!("f272670eb55e895584501d564afeb048bed26194"),
    protocol_versions_address: address!("79add5713b383daa0a138d3c4780c7a1804a8090"),
    superchain_config_address: Some(address!("C2Be75506d5724086DEB7245bd260Cc9753911Be")),
    da_challenge_address: None,
    blobs_enabled_l1_timestamp: None,
    interop_message_expiry_window: DEFAULT_INTEROP_MESSAGE_EXPIRY_WINDOW,
};
