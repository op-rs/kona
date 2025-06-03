use alloy_primitives::B256;
use async_trait::async_trait;
use op_alloy_consensus::OpReceiptEnvelope;
use std::fmt::Debug;

/// Collection of transaction receipts.
pub type Receipts = Vec<OpReceiptEnvelope>;

/// [`ReceiptProvider`] abstracts fetching receipts for a given block hash.
///
/// This trait exists to decouple the indexing logs from tightly coupling with
/// the full `ManagedModeApi`. It allows using a minimal provider
/// focused only on receipt access.
#[async_trait]
pub trait ReceiptProvider: Send + Sync + Debug {
    /// Associated Error Type to avoid dependency
    type Error: Debug + Send + Sync + 'static;

    /// Fetch all transaction receipts for the block with the given hash.
    ///
    /// # Arguments
    /// * `block_hash` - The hash of the block whose receipts should be fetched.
    ///
    /// # Returns
    /// A vector of [`OpReceiptEnvelope`]s representing all transaction receipts in the block,
    /// or an error if the fetch fails.
    async fn fetch_receipts(&self, block_hash: B256) -> Result<Receipts, Self::Error>;
}
