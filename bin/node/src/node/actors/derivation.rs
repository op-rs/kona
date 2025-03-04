//! [NodeActor] implementation for the derivation sub-routine.

use crate::node::{NodeActor, NodeEvent};
use async_trait::async_trait;
use kona_derive::traits::{OriginAdvancer, Pipeline, SignalReceiver};

/// The [NodeActor] for the derivation sub-routine.
///
/// This actor is responsible for receiving messages from [NodeProducer]s and stepping the
/// derivation pipeline forward to produce new payload attributes. The actor then sends the payload
/// to the [NodeActor] responsible for the execution sub-routine.
///
/// [NodeProducer]: crate::node::NodeProducer
pub struct DerivationActor<P>
where
    P: Pipeline + OriginAdvancer + SignalReceiver,
{
    _pipeline: P,
}

#[async_trait]
impl<P> NodeActor for DerivationActor<P>
where
    P: Pipeline + OriginAdvancer + SignalReceiver + Send + Sync,
{
    type Error = ();
    type Event = NodeEvent;

    async fn start(self) -> Result<(), Self::Error> {
        unimplemented!()
    }

    async fn process(&mut self, _msg: Self::Event) -> Result<(), Self::Error> {
        unimplemented!()
    }
}
