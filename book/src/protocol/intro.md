# Kona Protocol Libraries

The Kona monorepo contains a set of protocol crates that are designed
to be `no_std` compatible for Kona's fault proof sdk. Protocol crates
are built on [alloy][alloy] and [op-alloy][op-alloy] types.

The following protocol crates are published to [crates.io][crates].

<a href="https://crates.io/crates/kona-hardforks"><img src="https://img.shields.io/crates/v/kona-hardforks.svg" alt="kona-hardforks crate"></a>
<a href="https://crates.io/crates/kona-registry"><img src="https://img.shields.io/crates/v/kona-registry.svg" alt="kona-registry crate"></a>
<a href="https://crates.io/crates/kona-protocol"><img src="https://img.shields.io/crates/v/kona-protocol.svg" alt="kona-protocol crate"></a>
<a href="https://crates.io/crates/kona-genesis"><img src="https://img.shields.io/crates/v/kona-genesis.svg" alt="kona-genesis crate"></a>
<a href="https://crates.io/crates/kona-interop"><img src="https://img.shields.io/crates/v/kona-interop.svg" alt="kona-interop crate"></a>

At the lowest level, `kona-genesis` and `kona-hardforks` expose
core genesis and hardfork types.

`kona-protocol` sits just above `kona-genesis`, composing genesis types
into other core protocol types, as well as many independent protocol types.

More recently, the `kona-interop` crate was introduced that contains types
specific to the [Interop][interop] project.

Aside from types, Kona contains `kona-registry` that are bindings to the
[superchain-registry][scr]. The registry is available in a `no_std` environment
but requires `serde` to read serialized configs at compile time. Types
deserialized at compile time by the registry are from `kona-genesis`.


<!-- Links -->

[crates]: https://crates.io
[alloy]: https://github.com/alloy-rs/alloy
[op-alloy]: https://github.com/alloy-rs/op-alloy
[interop]: https://specs.optimism.io/interop/overview.html
[scr]: https://github.com/ethereum-optimism/superchain-registry
