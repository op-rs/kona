# Protocol

<a href="https://crates.io/crates/maili-protocol"><img src="https://img.shields.io/crates/v/maili-protocol.svg" alt="maili-protocol crate"></a>

The [`maili-protocol`][protocol] crate contains types, constants, and methods
specific to Optimism derivation and batch-submission.

[`maili-protocol`][protocol] supports `no_std`.

## Background

Protocol types are primarily used for L2 chain derivation. This section will
break down L2 chain derivation as it relates to types defined in
`maili-protocol` - that is, from the raw L2 chain data posted to L1, to the
[`Batch`][batch] type. And since the [`Batch`][batch] type naively breaks up
into the payload attributes, once executed, it becomes the canonical L2 block!
Note though, this provides an incredibly simplified introduction. It is advised
to reference [the specs][s] for the most up-to-date information regarding
derivation.

The L2 chain is derived from data posted to the L1 chain - either as calldata
or blob data. Data is iteratively pulled from each L1 block and translated
into the first type defined by `maili-protocol`: the [`Frame`][frame] type.

[`Frame`][frame]s are [parsed][parsed] from the raw data. Each [`Frame`][frame]
is a part of a [`Channel`][channel], the next type one level up in deriving
L2 blocks. [`Channel`][channel]s have IDs that frames reference. [`Frame`][frame]s
are [added][added] iteratively to the [`Channel`][channel]. Once a
[`Channel`][channel] [is ready][ready], it can be used to read a [`Batch`][batch].

Since a [`Channel`][channel] stitches together frames, it contains the raw frame
data. In order to turn this [`Channel`][channel] data into a [`Batch`][batch],
it needs to be decompressed using the respective (de)compression algorithm
(see [the channel specs][channel-specs] for more detail on this). Once
decompressed, the raw data can be [decoded][decoded] into the [`Batch`][batch]
type.


## Sections

#### Core Derviation Types (discussed above)

- [Frames](./frames.md)
- [Channels](./channels.md)
- [Batches](./batches.md)

#### Other Critical Protocol Types

- [BlockInfo and L2BlockInfo Types](./block-info.md)
- [L1BlockInfo Types](./l1-block-info.md)


<!-- Links -->

[decoded]: https://docs.rs/maili-protocol/latest/maili_protocol/enum.Batch.html#method.decode
[batch]: https://docs.rs/maili-protocol/latest/maili_protocol/enum.Batch.html
[ready]: https://docs.rs/maili-protocol/latest/maili_protocol/struct.Channel.html#method.is_ready
[added]: https://docs.rs/maili-protocol/latest/maili_protocol/struct.Channel.html#method.add_frame
[channel]: https://docs.rs/maili-protocol/latest/maili_protocol/struct.Channel.html
[frame]: https://docs.rs/maili-protocol/latest/maili_protocol/struct.Frame.html
[parsed]: https://docs.rs/maili-protocol/latest/maili_protocol/struct.Frame.html#method.parse_frames

[protocol]: https://crates.io/crates/maili-protocol
[s]: https://specs.optimism.io/protocol/derivation.html#overview
[lcd]: https://specs.optimism.io/protocol/derivation.html#overview
[channel-specs]: https://specs.optimism.io/protocol/derivation.html#channel-format
