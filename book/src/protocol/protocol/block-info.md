# BlockInfo and L2BlockInfo Types

Optimism defines block info types that encapsulate minimal block header
information needed by protocol operations.


## BlockInfo

The [`BlockInfo`][bi] type is straightforward, containing the block hash,
number, parent hash, and timestamp.


## L2BlockInfo

The [`L2BlockInfo`][lbi] extends the [`BlockInfo`][bi] type for the canonical
L2 chain. It contains the "L1 origin" which is a set of block info for the L1
block that this L2 block "originated".

[`L2BlockInfo`][lbi] provides a [`from_block_and_gensis`][fbg] method to
construct the [`L2BlockInfo`][lbi] from a block and `ChainGenesis`.


<!-- Links -->

[bi]: https://docs.rs/maili-protocol/latest/maili_protocol/struct.BlockInfo.html
[lbi]: https://docs.rs/maili-protocol/latest/maili_protocol/struct.L2BlockInfo.html
[fbg]: https://docs.rs/maili-protocol/latest/maili_protocol/struct.L2BlockInfo.html#method.from_block_and_genesis
