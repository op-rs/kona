## `kona-genesis`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-genesis"><img src="https://img.shields.io/crates/v/kona-genesis.svg" alt="kona-genesis crate"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="MIT License"></a>
<a href="https://rollup.yoga"><img src="https://img.shields.io/badge/Docs-854a15?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=mdBook&logoColor=BEC5C9" alt="Docs" /></a>


Genesis configuration types for OP Stack chains.

This crate provides essential configuration types that define the initial state 
and parameters of OP Stack rollups. It serves as the source of truth for chain 
configuration across all Kona components.

## Overview

The genesis configuration defines:
- **Chain Parameters**: Block time, sequencer addresses, fee configuration
- **L1 Integration**: Addresses of L1 contracts that power the rollup
- **Upgrade Schedule**: Hardfork activation blocks and timestamps
- **System Config**: Initial values for gas limits, scalars, and other parameters

### Usage

_By default, `kona-genesis` enables both `std` and `serde` features._

If you're working in a `no_std` environment (like [`kona`][kona]), disable default features like so.

```toml
[dependencies]
kona-genesis = { version = "x.y.z", default-features = false, features = ["serde"] }
```

#### Rollup Config

`kona-genesis` exports a `RollupConfig`, the primary genesis type for Optimism Consensus.

```rust,ignore
use kona_genesis::{RollupConfig, ChainGenesis};

// Load configuration for a specific chain
let config = RollupConfig::from_l2_chain_id(10)?; // OP Mainnet

// Access chain parameters
let block_time = config.block_time;
let l1_chain_id = config.l1_chain_id;

// Check upgrade status
if config.is_fjord_active(timestamp) {
    // Use Fjord-specific logic
}
```

#### System Config

The `SystemConfig` type tracks dynamic L2 chain parameters that can be updated via L1:

```rust,ignore
use kona_genesis::SystemConfig;

// Parse system config from L1 transaction
let sys_config = SystemConfig::from_l1_tx(&tx)?;

// Access current parameters
let gas_limit = sys_config.gas_limit;
let base_fee_scalar = sys_config.base_fee_scalar;
```

<!-- Links -->

[alloy-genesis]: https://github.com/alloy-rs
[kona]: https://github.com/op-rs/kona/blob/main/Cargo.toml#L137
