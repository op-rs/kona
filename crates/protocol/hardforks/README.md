# `kona-hardforks`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-hardforks"><img src="https://img.shields.io/crates/v/kona-hardforks.svg" alt="kona-hardforks crate"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="MIT License"></a>
<a href="https://rollup.yoga"><img src="https://img.shields.io/badge/Docs-854a15?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=mdBook&logoColor=BEC5C9" alt="Docs" /></a>

Consensus layer hardfork types and network upgrade transactions for the OP Stack.

## Overview

This crate manages hardfork activations and network upgrades across OP Stack chains.
It provides types and utilities for handling protocol changes that require coordinated
upgrades across the network.

## Hardfork Types

### Supported Hardforks

- **Bedrock**: The foundational OP Stack upgrade
- **Regolith**: Optimizations and bug fixes
- **Canyon**: EIP-1559 style transactions and base fee
- **Delta**: Span batch support for improved compression
- **Ecotone**: 4844 blob support and Dencun compatibility
- **Fjord**: Additional optimizations and protocol improvements
- **Granite**: Latest protocol enhancements
- **Holocene**: Upcoming features (when activated)

### Upgrade Transactions

Special system transactions that activate hardfork changes:
- L1 block info updates
- Gas parameter adjustments  
- System config modifications
- Beacon block root updates (post-Ecotone)

## Usage

```rust,ignore
use kona_hardforks::{Hardforks, UpgradeTransactions};
use kona_genesis::RollupConfig;

// Check active hardforks
let config = RollupConfig::default();
if config.is_fjord_active(timestamp) {
    // Apply Fjord-specific logic
}

// Generate upgrade transactions
let txs = UpgradeTransactions::for_timestamp(&config, timestamp);
for tx in txs {
    // Include in block at upgrade boundary
}
```

## Upgrade Coordination

Hardforks require careful coordination:
1. **Timestamp-based activation**: All nodes activate at the same time
2. **Upgrade transactions**: Special txs mark the upgrade block
3. **State transitions**: Protocol rules change at activation
4. **Backwards compatibility**: Handle pre/post fork blocks correctly

## Safety

- Always use the rollup config to check hardfork status
- Include all required upgrade transactions at activation
- Validate blocks according to their hardfork rules
- Test thoroughly across hardfork boundaries

### Provenance

This code was ported [op-alloy] as part of `kona` monorepo migrations.

[op-alloy]: https://github.com/alloy-rs/op-alloy
