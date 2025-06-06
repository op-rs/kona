//! Contains batch types.

mod r#type;
pub use r#type::*;

mod reader;
pub use reader::{BatchReader, DecompressionError};

mod tx;
pub use tx::BatchTransaction;

mod core;
pub use core::Batch;

mod raw;
pub use raw::RawSpanBatch;

mod payload;
pub use payload::SpanBatchPayload;

mod prefix;
pub use prefix::SpanBatchPrefix;

mod inclusion;
pub use inclusion::BatchWithInclusionBlock;

mod errors;
pub use errors::{BatchDecodingError, BatchEncodingError, SpanBatchError, SpanDecodingError};

mod bits;
pub use bits::SpanBatchBits;

mod span;
pub use span::SpanBatch;

mod transactions;
pub use transactions::SpanBatchTransactions;

mod element;
pub use element::{MAX_SPAN_BATCH_ELEMENTS, SpanBatchElement};

mod validity;
pub use validity::BatchValidity;

mod single;
pub use single::SingleBatch;

mod tx_data;
pub use tx_data::{
    SpanBatchEip1559TransactionData, SpanBatchEip2930TransactionData,
    SpanBatchEip7702TransactionData, SpanBatchLegacyTransactionData, SpanBatchTransactionData,
};

mod traits;
pub use traits::BatchValidationProvider;
