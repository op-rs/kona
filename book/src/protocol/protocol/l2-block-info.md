# L2BlockInfo

The [`L2BlockInfo`][lbi] extends the [`BlockInfo`][bi] type for the canonical
L2 chain. It contains the "L1 origin" which is a set of block info for the L1
block that this L2 block "originated".

Similarly to the [`BlockInfo`][bi] type, `L2BlockInfo` is a subset of information
provided by a block header, used for protocol operations.

[`L2BlockInfo`][lbi] provides a [`from_block_and_gensis`][fbg] method to
construct the [`L2BlockInfo`][lbi] from a block and `ChainGenesis`.


<!-- Links -->

[bi]: https://docs.rs/kona-protocol/latest/kona_protocol/struct.BlockInfo.html
[lbi]: https://docs.rs/kona-protocol/latest/kona_protocol/struct.L2BlockInfo.html
[fbg]: https://docs.rs/kona-protocol/latest/kona_protocol/struct.L2BlockInfo.html#method.from_block_and_genesis
