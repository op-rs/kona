## `kona-registry`

<a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
<a href="https://crates.io/crates/kona-registry"><img src="https://img.shields.io/crates/v/kona-registry.svg?label=kona-registry&labelColor=2a2f35" alt="kona-registry"></a>
<a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="MIT License"></a>
<a href="https://rollup.yoga"><img src="https://img.shields.io/badge/Docs-854a15?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=mdBook&logoColor=BEC5C9" alt="Docs" /></a>

[`kona-registry`][sc] is a `no_std` crate that exports rust type definitions for chains
in the [`superchain-registry`][osr]. Since it reads static files to read configurations for
various chains into instantiated objects, the [`kona-registry`][sc] crate requires
[`serde`][serde] as a dependency. To use the [`kona-registry`][sc] crate, add the crate
as a dependency to a `Cargo.toml`.

```toml
kona-registry = "0.1.0"
```

[`kona-registry`][sc] declares lazy evaluated statics that expose `ChainConfig`s, `RollupConfig`s,
and `Chain` objects for all chains with static definitions in the superchain registry. The way this works
is the golang side of the superchain registry contains an "internal code generation" script that has
been modified to output configuration files to the [`crates/registry`][s] directory in the
`etc` folder that are read by the [`kona-registry`][sc] rust crate. These static config files
contain an up-to-date list of all superchain configurations with their chain configs. It is expected
that if the commit hash of the [`superchain-registry`][osr] pulled in as a git submodule has breaking
changes, the tests in this crate (`kona-registry`) will break and updates will need to be made.

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


### Feature Flags

- `std`: Uses the standard library to pull in environment variables.


### Credits

[superchain-registry][osr] contributors for building and maintaining superchain types.

[alloy] and [op-alloy] for creating and maintaining high quality Ethereum and Optimism types in rust.


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
