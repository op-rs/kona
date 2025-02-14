# Transform Frames into a Batch

> [!NOTE]
>
> This example performs the reverse transformation as the [batch-to-frames][batch-to-frames] example.

> [!CAUTION]
>
> Steps and handling of types with respect to chain tip, ordering of frames, re-orgs, and
> more are not covered by this example. This example solely demonstrates the most trivial
> way to transform individual [`Frame`][frame]s into a [`Batch`][batch] type.

This example walks through transforming [`Frame`][frame]s into the [`Batch`][batch] types.

## Walkthrough

The high level transformation is the following.

```
raw bytes[] -> frames[] -> channel -> decompressed channel data -> Batch
```

Given the raw, batch-submitted frame data as bytes (read in with the [`hex!` macro][hex]),
the first step is to decode the frame data into [`Frame`][frame]s using
[`Frame::decode`][decode-frame]. Once all the [`Frame`][frame]s are decoded,
the [`Channel`][channel] can be constructed using the [`ChannelId`][channel-id]
of the first frame.

> [!Note]
>
> [`Frame`][frame]s may also be added to a [`Channel`][channel]
> once decoded with the [`Channel::add_frame`][add-frame] method.

When the [`Channel`][channel] is [`Channel::is_ready()`][is-ready],
the frame data can taken from the [`Channel`][channel] using
[`Channel::frame_data()`][frame-data]. This data is represented as [`Bytes`][bytes]
and needs to be decompressed using the respective compression algorithm depending on
which hardforks are activated (using the `RollupConfig`). For the sake of this example,
`brotli` is used (which was activated in the [Fjord hardfork][fjord]). Decompressed
brotli bytes can then be passed right into [`Batch::decode`][decode-batch]
to wind up with the example's desired [`Batch`][batch].


## Running this example:

- Clone the examples repository: `git clone git@github.com:op-rs/maili.git`
- Run: `cargo run --example frames_to_batch`

```rust
{{#include ../../../crates/protocol/examples/frames_to_batch.rs}}
```

<!-- Links -->

[batch-to-frames]: ./batch-to-frames.md

{{#include ../links.md}}
