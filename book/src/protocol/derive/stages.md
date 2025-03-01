# Swapping out a Stage

In the [introduction to the derivation pipeline][intro], the derivation pipeline
is broken down to demonstrate the composition of stages, forming the transformation
function from L1 data into L2 payload attributes.

What makes kona's derivation pipeline extensible is that stages are composed using
trait-abstraction. That is, each successive stage composes the previous stage as
a generic. As such as long as a stage satisfies two rules, it can be swapped into
the pipeline seamlessly.
1. The stage implements the trait required by the next stage.
2. The stage uses the same trait for the previous stage as the
   current stage to be swapped out.

Below provides a concrete example, swapping out the `L1Retrieval` stage.

## Example

In the current, post-Holocene hardfork [`DerivationPipeline`][dp], the bottom three
stages of the pipeline are as follows (from top down).

- [`FrameQueue`][frame-queue]
- [`L1Retrieval`][retrieval]
- [`L1Traversal`][traversal]

In this set of stages, the [`L1Traversal`][traversal] stage sits at the bottom.
It implements the [`L1Retrieval`][retrieval] trait called the
[`L1RetrievalProvider`][retrieval-provider]. This provides generic methods that
allow the [`L1Retrieval`][retrieval] stage to call those methods on the generic
previous stage that implements this provider trait.

As we go up a level, the same trait abstraction occurs. The [`L1Retrieval`][retrieval]
stage implements the provider trait that the [`FrameQueue`][frame-queue] stage requires.
This trait is the [`FrameQueueProvider`][frame-queue-provider].

Now that we understand the trait abstractions, let's swap out the
[`L1Retrieval`][retrieval] stage for a custom `DapRetrieval` stage.

```rust
// ...
// imports
// ...

// We use the same "L1RetrievalProvider" trait here
// in order to seamlessly use the `L1Traversal`

/// DapRetrieval stage
#[derive(Debug)]
pub struct DapRetrieval<P>
where
    P: L1RetrievalProvider + OriginAdvancer + OriginProvider + SignalReceiver,
{
    /// The previous stage in the pipeline.
    pub prev: P,
    provider: YourDataAvailabilityProvider,
    data: Option<Bytes>,
}

#[async_trait]
impl<P> FrameQueueProvider for DapRetrieval<P>
where
    P: L1RetrievalProvider + OriginAdvancer + OriginProvider + SignalReceiver + Send,
{
    type Item = Bytes;

    async fn next_data(&mut self) -> PipelineResult<Self::Item> {
        if self.data.is_none() {
            let next = self
                .prev
                .next_l1_block()
                .await? // SAFETY: This question mark bubbles up the Eof error.
                .ok_or(PipelineError::MissingL1Data.temp())?;
            self.data = Some(self.provider.get_data(&next).await?);
        }

        match self.data.as_mut().expect("Cannot be None").next().await {
            Ok(data) => Ok(data),
            Err(e) => {
                if let PipelineErrorKind::Temporary(PipelineError::Eof) = e {
                    self.data = None;
                }
                Err(e)
            }
        }
    }
}

// ...
// impl OriginAdvancer for DapRetrieval
// impl OriginProvider for DapRetrieval
// impl SignalReceiver for DapRetrieval
// ..
```

Notice, the `L1RetrievalProvider` is used as a trait bound so the
[`L1Traversal`][traversal] stage can be used seamlessly as the "prev" stage in the pipeline.
Concretely, an instantiation of the `DapRetrieval` stage could be the following.

```
DapRetrieval<L1Traversal<..>>
```


<!-- Links -->

[intro]: ./intro.md
[dp]: https://docs.rs/kona-derive/latest/kona_derive/pipeline/struct.DerivationPipeline.html
[retrieval-provider]: https://docs.rs/kona-derive/latest/kona_derive/stages/trait.L1RetrievalProvider.html
[frame-queue-provider]: https://docs.rs/kona-derive/latest/kona_derive/stages/trait.FrameQueueProvider.html

[frame-queue]: https://docs.rs/kona-derive/latest/kona_derive/stages/struct.FrameQueue.html
[retrieval]: https://docs.rs/kona-derive/latest/kona_derive/stages/struct.L1Retrieval.html
[traversal]: https://docs.rs/kona-derive/latest/kona_derive/stages/struct.L1Traversal.html
