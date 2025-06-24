#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "metrics"), no_std)]

extern crate alloc;

#[macro_use]
extern crate tracing;

mod attributes;
pub use attributes::StatefulAttributesBuilder;

mod errors;
pub use errors::{
    BatchDecompressionError, BlobDecodingError, BlobProviderError, BuilderError,
    PipelineEncodingError, PipelineError, PipelineErrorKind, ResetError,
};

mod pipeline;
pub use pipeline::{
    AttributesQueueStage, BatchProviderStage, BatchStreamStage, ChannelProviderStage,
    ChannelReaderStage, DerivationPipeline, FrameQueueStage, L1RetrievalStage, L1TraversalStage,
    PipelineBuilder,
};

mod sources;
pub use sources::{BlobData, BlobSource, CalldataSource, EthereumDataSource};

mod stages;
pub use stages::{
    AttributesQueue, BatchProvider, BatchQueue, BatchStream, BatchStreamProvider, BatchValidator,
    ChannelAssembler, ChannelBank, ChannelProvider, ChannelReader, ChannelReaderProvider,
    FrameQueue, FrameQueueProvider, L1Retrieval, L1RetrievalProvider, L1Traversal,
    NextBatchProvider, NextFrameProvider,
};

mod traits;
pub use traits::{
    AttributesBuilder, AttributesProvider, BatchValidationProviderDerive, BlobProvider,
    ChainProvider, DataAvailabilityProvider, L2ChainProvider, NextAttributes, OriginAdvancer,
    OriginProvider, Pipeline, ResetProvider, SignalReceiver,
};

mod types;
pub use types::{ActivationSignal, PipelineResult, ResetSignal, Signal, StepResult};

mod metrics;
pub use metrics::Metrics;

#[cfg(feature = "test-utils")]
pub mod test_utils;
