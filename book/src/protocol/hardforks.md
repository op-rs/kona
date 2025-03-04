# Hardforks

<a href="https://crates.io/crates/kona-hardforks"><img src="https://img.shields.io/crates/v/kona-hardforks.svg" alt="kona-hardforks crate"></a>

Hardforks are consensus layer types of the OP Stack.

`kona-hardforks` most directly exports the [`Hardforks`][hardforks] type that provides
the network upgrade transactions for OP Stack hardforks including the following.
- [Ecotone][ecotone]
- [Fjord][fjord]
- [Isthmus][isthmus]

Each hardfork has its own type in `kona-hardforks` that exposes the network
upgrade transactions for that hardfork.

For example, the [`Ecotone`][ecotone-ty] type can be used to retrieve the Ecotone
network upgrade transactions through its [`txs() -> impl Iterator<Bytes>`][txs] method.

```rust
// Notice, the `Hardfork` trait must be imported in order to
// provide the `txs()` method implemented for the hardfork type.
use kona_hardforks::{Hardfork, Ecotone};
let ecotone_upgrade_txs = Ecotone.txs();
assert_eq!(ecotone_upgrade_txs.collect::<Vec<_>>().len(), 6);
```

Conveniently, the [`Hardforks`][hardforks] type exposes each hardfork type as a field
that can be directly referenced without needing to import all the different hardforks.

```rust
use kona_hardforks::{Hardfork, Hardforks};
let ecotone_upgrade_txs = Hardforks::ECOTONE.txs();
assert_eq!(ecotone_upgrade_txs.collect::<Vec<_>>().len(), 6);
```

<!-- Links -->

[fjord]: https://specs.optimism.io/protocol/fjord/overview.html
[ecotone]: https://specs.optimism.io/protocol/ecotone/overview.html
[isthmus]: https://specs.optimism.io/protocol/isthmus/overview.html

[ecotone-ty]: https://docs.rs/kona-hardforks/latest/kona_hardforks/struct.Ecotone.html
[hardforks]: https://docs.rs/kona-hardforks/latest/kona_hardforks/struct.Hardforks.html

[txs]: https://docs.rs/kona-hardforks/latest/kona_hardforks/struct.Ecotone.html#method.txs
