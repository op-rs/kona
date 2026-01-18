# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands
- Build workspace: `just b` or `just build-native`
- Build rollup node: `just build-node`
- Build supervisor: `just build-supervisor`
- Lint: `just l` or `just lint-native`
- Lint all targets (native, cannon, asterisc): `just lint-all`
- Format: `just f` or `just fmt-native-fix`
- Run all tests: `just t` or `just tests`
- Run specific test: `cargo nextest run --package [package-name] --test [test-name]`
- Run single test: `cargo nextest run --package [package-name] --test [test-name] -- [test_function_name]`
- Run online tests (excluded by default): `just test-online`
- Documentation tests: `just test-docs`
- Check spelling: `just lint-typos` (requires `typos-cli`)
- Feature powerset check: `just hack` (requires `cargo-hack`)

## Code Style
- MSRV: 1.88
- Format with nightly rustfmt: `cargo +nightly fmt`
- All clippy warnings are treated as errors (-D warnings)
- Imports: organized by crate, reordered automatically (`imports_granularity = "Crate"`)

## Architecture Overview

Kona is a monorepo for OP Stack types, components, and services built in Rust. The repository is organized into several major categories:

### Binaries (`bin/`)
- **`client`**: The fault proof program that executes state transitions on a prover
- **`host`**: Native program serving as the Preimage Oracle server
- **`node`**: Rollup Node implementation with flexible chain ID support
- **`supervisor`**: Supervisor implementation for interop coordination

### Protocol (`crates/protocol/`)
- **`derive`**: `no_std` compatible derivation pipeline implementation
- **`protocol`**: Core protocol types used across OP Stack rust crates
- **`genesis`**: Genesis types for OP Stack chains
- **`interop`**: Core functionality for OP Stack Interop features
- **`registry`**: Rust bindings for superchain-registry
- **`hardforks`**: Consensus layer hardfork types and network upgrade transactions

### Proof (`crates/proof/`)
- **`executor`**: `no_std` stateless block executor
- **`proof`**: High-level OP Stack state transition proof SDK
- **`proof-interop`**: Extension of `kona-proof` with interop support
- **`mpt`**: Merkle Patricia Trie utilities for client program
- **`preimage`**: High-level PreimageOracle ABI interfaces
- **`std-fpvm`**: Platform-specific Fault Proof VM kernel APIs
- **`driver`**: Stateful derivation pipeline driver

### Node (`crates/node/`)
- **`service`**: OP Stack rollup node service implementation
- **`engine`**: Extensible rollup node engine client
- **`rpc`**: OP Stack RPC types and extensions
- **`gossip`**: P2P Gossip networking
- **`disc`**: P2P Discovery networking
- **`peers`**: Networking utilities ported from reth
- **`sources`**: Data source types and utilities

### Supervisor (`crates/supervisor/`)
- **`core`**: Core supervisor functionality
- **`service`**: Supervisor service implementation
- **`rpc`**: Supervisor RPC types and client
- **`storage`**: Database storage layer (uses reth-db)
- **`types`**: Common types for supervisor components

### Batcher (`crates/batcher/`)
- **`comp`**: Compression types and utilities for the OP Stack

### Providers (`crates/providers/`)
- **`providers-alloy`**: Provider implementations backed by Alloy
- **`providers-local`**: Local provider implementations

### Utilities (`crates/utilities/`)
- **`cli`**: Standard CLI utilities used across binaries
- **`serde`**: Serialization helpers
- **`macros`**: Utility macros

## Cross-Compilation Targets

The proof components support `no_std` and can be cross-compiled for fault proof VMs:
- **cannon**: MIPS64 target - `just build-cannon-client`
- **asterisc**: RISC-V target - `just build-asterisc-client`

Linting for these targets requires Docker:
- `just lint-cannon`
- `just lint-asterisc`

## E2E and Acceptance Tests

Tests are located in `tests/` directory with its own justfile:
- Action tests for single-chain: `just action-tests-single`
- Action tests for interop: `just action-tests-interop`
- E2E tests with sysgo orchestrator: `just test-e2e-sysgo`
- Acceptance tests: `just acceptance-tests`

These require building client binaries and setting up the Optimism monorepo submodule.

## Key Dependencies
- **Alloy ecosystem**: Ethereum types and providers
- **op-alloy**: OP Stack extensions to Alloy
- **revm/op-revm**: EVM execution
- **reth**: Database types (for supervisor storage)
