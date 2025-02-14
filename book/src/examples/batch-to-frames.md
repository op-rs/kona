# Transform a Batch into Frames

> [!NOTE]
>
> This example performs the reverse transformation as the [frames-to-batch][frames-to-batch] example.

> [!CAUTION]
>
> Steps and handling of types with respect to chain tip, ordering of frames, re-orgs, and
> more are not covered by this example. This example solely demonstrates the most trivial
> way to transform an individual [`Batch`][batch] into [`Frame`][frame]s.

This example walks through transforming a [`Batch`][batch] into [`Frame`][frame]s.

Effectively, this example demonstrates the _encoding_ process from an L2 batch into the
serialized bytes that are posted to the data availability layer.

## Walkthrough

The high level transformation is the following.

```
Batch -> decompressed batch data -> ChannelOut -> frames[] -> bytes[]
```

Given the [`Batch`][batch], the first step to encode the batch
using the [`Batch::encode()`][encode-batch] method. The output bytes
need to then be compressed prior to adding them to the
[`ChannelOut`][channel-out].

> [!NOTE]
>
> The [`ChannelOut`][channel-out] type also provides a method for adding
> the [`Batch`][batch] itself, handling encoding and compression, but
> this method is not available yet.

Once compressed using the [`compress_brotli`][compress-brotli] method, the
compressed bytes can be added to a newly constructed [`ChannelOut`][channel-out].
As long as the [`ChannelOut`][channel-out] has [`ready_bytes()`][ready-bytes],
[`Frame`][frame]s can be constructed using the
[`ChannelOut::output_frame()`][output-frame] method, specifying the maximum
frame size.

Once [`Frame`][frame]s are returned from the [`ChannelOut`][channel-out],
they can be [`Frame::encode`][encode-frame] into raw, serialized data
ready to be batch-submitted to the data-availability layer.


## Running this example:

- Clone the examples repository: `git clone git@github.com:op-rs/maili.git`
- Run: `cargo run --example batch_to_frames`

```rust
{{#include ../../../crates/protocol/examples/batch_to_frames.rs}}
```

<!-- Links -->

[frames-to-batch]: ./frames-to-batch.md

{{#include ../links.md}}
