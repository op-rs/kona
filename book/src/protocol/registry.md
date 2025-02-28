# Registry

<a href="https://crates.io/crates/kona-registry"><img src="https://img.shields.io/crates/v/kona-registry.svg" alt="kona-registry crate"></a>

[`kona-registry`][sc] is a `no_std` crate that exports rust type definitions for chains
in the [`superchain-registry`][osr]. These are lazily evaluated statics that provide
`ChainConfig`s, `RollupConfig`s, and `Chain` objects for all chains with static definitions
in the [`superchain-registry`][osr].

Since it reads static files to read configurations for various chains into instantiated
objects, the [`kona-registry`][sc] crate requires [`serde`][serde] as a dependency.

There are three core statics exposed by the [`kona-registry`][sc].
- `CHAINS`: A list of chain objects containing the superchain metadata for this chain.
- `OPCHAINS`: A map from chain id to `ChainConfig`.
- `ROLLUP_CONFIGS`: A map from chain id to `RollupConfig`.

[`kona-registry`][sc] exports the _complete_ list of chains within the superchain, as well as each
chain's `RollupConfig`s and `ChainConfig`s.

### Usage

Add the following to your `Cargo.toml`.

```toml
[dependencies]
kona-registry = "0.1.0"
```

To make `kona-registry` `no_std`, toggle `default-features` off like so.

```toml
[dependencies]
kona-registry = { version = "0.1.0", default-features = false }
```

Below demonstrates getting the `RollupConfig` for OP Mainnet (Chain ID `10`).

```rust
use kona_registry::ROLLUP_CONFIGS;

let op_chain_id = 10;
let op_rollup_config = ROLLUP_CONFIGS.get(&op_chain_id);
println!("OP Mainnet Rollup Config: {:?}", op_rollup_config);
```

A mapping from chain id to `ChainConfig` is also available.

```rust
use kona_registry::OPCHAINS;

let op_chain_id = 10;
let op_chain_config = OPCHAINS.get(&op_chain_id);
println!("OP Mainnet Chain Config: {:?}", op_chain_config);
```

<!-- Hyperlinks -->

[serde]: https://crates.io/crates/serde
[alloy]: https://github.com/alloy-rs/alloy
[op-alloy]: https://github.com/alloy-rs/op-alloy
[op-superchain]: https://docs.optimism.io/stack/explainer
[osr]: https://github.com/ethereum-optimism/superchain-registry

[s]: ./crates/registry
[sc]: https://crates.io/crates/kona-registry
[g]: https://crates.io/crates/kona-genesis

[chains]: https://docs.rs/kona-registry/latest/superchain/struct.CHAINS.html
[opchains]: https://docs.rs/kona-registry/latest/superchain/struct.OPCHAINS.html
[rollups]: https://docs.rs/kona-registry/latest/superchain/struct.ROLLUP_CONFIGS.html
