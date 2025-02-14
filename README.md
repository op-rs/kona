<h1 align="center">
<img src="./assets/banner.png" alt="Kona" width="100%" align="center">
</h1>

<h4 align="center">
    A verifiable implementation of the <a href="https://github.com/ethereum-optimism/optimism">Optimism</a> rollup state transition.
</h4>

<p align="center">
  <a href="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml"><img src="https://github.com/op-rs/kona/actions/workflows/rust_ci.yaml/badge.svg?label=ci" alt="CI"></a>
  <a href="https://github.com/op-rs/kona/blob/main/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?label=license&labelColor=2a2f35" alt="License"></a>
  <a href="https://op-rs.github.io/kona"><img src="https://img.shields.io/badge/Contributor%20Book-854a15?logo=mdBook&labelColor=2a2f35" alt="Book"></a>
  <a href="https://github.com/ethereum-optimism/monorepo"><img src="https://img.shields.io/badge/OP%20Stack-monorepo-red?labelColor=2a2f35" alt="OP Stack"></a>
  <a href="https://img.shields.io/codecov/c/github/op-rs/kona"><img src="https://img.shields.io/codecov/c/github/op-rs/kona" alt="Codecov"></a>
</p>

<p align="center">
  <a href="#whats-kona">What's Kona?</a> •
  <a href="#overview">Overview</a> •
  <a href="https://op-rs.github.io/kona/CONTRIBUTING.html">Contributing</a> •
  <a href="#credits">Credits</a>
</p>

## What's Kona?

Kona is a suite of portable implementations of the OP Stack rollup state transition, namely the [derivation pipeline][g-derivation-pipeline] and
the block execution logic.

Built on top of these libraries, this repository also features a [fault proof program][fpp-specs] designed to deterministically execute the
rollup state transition in order to verify an [L2 output root][g-output-root] from the L1 inputs it was [derived from][g-derivation-pipeline].

### Alternative Backends

Kona's libraries were built with alternative backend support and extensibility in mind - it is not just a fault proof
program! Kona is also used by:

- [`op-succinct`][op-succinct]
- [`kailua`][kailua]

To build your own backend for kona, or build a new application on top of its libraries,
see the [SDK section of the book](https://op-rs.github.io/kona/sdk/intro.html).

### Development Status

`kona` is currently in active development, and is not yet ready for use in production.

## Overview

**Binaries**

- [`client`](./bin/client): The bare-metal program that runs on top of a [fault proof VM][g-fault-proof-vm].
- [`host`](./bin/host): The host program that runs natively alongside the FPVM, serving as the [Preimage Oracle][g-preimage-oracle] server.

**Protocol**

- [`derive`](./crates/protocol/derive): `no_std` compatible implementation of the [derivation pipeline][g-derivation-pipeline].
  - [`driver`](./crates/protocol/driver): Stateful derivation pipeline driver.
- [`interop`](./crates/protocol/interop): Core functionality and primitives for the [Interop feature](https://specs.optimism.io/interop/overview.html) of the OP Stack.

**Proof**

- [`mpt`](./crates/proof/mpt): Utilities for interacting with the Merkle Patricia Trie in the client program.
- [`executor`](./crates/proof/executor): `no_std` stateless block executor for the [OP Stack][op-stack].
- [`kona-proof`](./crates/proof/proof): High level OP Stack state transition proof SDK.
- [`kona-proof-interop`](./crates/proof/proof-interop): Extension of `kona-proof` with interop support.
- [`preimage`](./crates/proof/preimage): High level interfaces to the [`PreimageOracle`][fpp-specs] ABI.
- [`std-fpvm`](./crates/proof/std-fpvm): Platform specific [Fault Proof VM][g-fault-proof-vm] kernel APIs.
- [`std-fpvm-proc`](./crates/proof/std-fpvm-proc): Proc macro for [Fault Proof Program][fpp-specs] entrypoints.

## Book

The [book][book] contains a more in-depth overview of the project, contributor guidelines, tutorials for
getting started with building your own programs, and a reference for the libraries and tools provided by Kona.

## Credits

`kona` is inspired by the work of several teams, namely [OP Labs][op-labs] and other contributors' work on the
[`op-program`][op-program] and [BadBoiLabs][bad-boi-labs]'s work on [Cannon-rs][badboi-cannon-rs].

`kona` is also built on rust types in [alloy][alloy], [op-alloy][op-alloy], and [maili][maili].

[alloy]: https://github.com/alloy-rs/alloy
[maili]: https://github.com/op-rs/maili
[op-alloy]: https://github.com/alloy-rs/op-alloy
[op-stack]: https://github.com/ethereum-optimism/optimism
[op-program]: https://github.com/ethereum-optimism/optimism/tree/develop/op-program
[cannon]: https://github.com/ethereum-optimism/optimism/tree/develop/cannon
[cannon-rs]: https://github.com/op-rs/cannon-rs
[badboi-cannon-rs]: https://github.com/BadBoiLabs/cannon-rs
[asterisc]: https://github.com/etheruem-optimism/asterisc
[fpp-specs]: https://specs.optimism.io/fault-proof/index.html
[book]: https://op-rs.github.io/kona/
[op-succinct]: https://github.com/succinctlabs/op-succinct
[kailua]: https://github.com/risc0/kailua
[op-labs]: https://github.com/ethereum-optimism
[bad-boi-labs]: https://github.com/BadBoiLabs
[g-output-root]: https://specs.optimism.io/glossary.html#l2-output-root
[g-derivation-pipeline]: https://specs.optimism.io/protocol/derivation.html#l2-chain-derivation-pipeline
[g-fault-proof-vm]: https://specs.optimism.io/experimental/fault-proof/index.html#fault-proof-vm
[g-preimage-oracle]: https://specs.optimism.io/fault-proof/index.html#pre-image-oracle
