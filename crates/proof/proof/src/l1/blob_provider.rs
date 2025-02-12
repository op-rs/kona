//! Contains the concrete implementation of the [BlobProvider] trait for the client program.

use crate::{errors::OracleProviderError, HintType};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use alloy_consensus::Blob;
use alloy_eips::eip4844::{IndexedBlobHash, FIELD_ELEMENTS_PER_BLOB};
use alloy_primitives::keccak256;
use async_trait::async_trait;
use kona_derive::traits::BlobProvider;
use kona_preimage::{CommsClient, PreimageKey, PreimageKeyType};
use maili_protocol::BlockInfo;

/// An oracle-backed blob provider.
#[derive(Debug, Clone)]
pub struct OracleBlobProvider<T: CommsClient> {
    oracle: Arc<T>,
}

impl<T: CommsClient> OracleBlobProvider<T> {
    /// Constructs a new `OracleBlobProvider`.
    pub const fn new(oracle: Arc<T>) -> Self {
        Self { oracle }
    }

    /// Retrieves a blob from the oracle.
    ///
    /// ## Takes
    /// - `block_ref`: The block reference.
    /// - `blob_hash`: The blob hash.
    ///
    /// ## Returns
    /// - `Ok(blob)`: The blob.
    /// - `Err(e)`: The blob could not be retrieved.
    async fn get_blob(
        &self,
        block_ref: &BlockInfo,
        blob_hash: &IndexedBlobHash,
    ) -> Result<Blob, OracleProviderError> {
        let mut blob_req_meta = [0u8; 48];
        blob_req_meta[0..32].copy_from_slice(blob_hash.hash.as_ref());
        blob_req_meta[32..40].copy_from_slice((blob_hash.index).to_be_bytes().as_ref());
        blob_req_meta[40..48].copy_from_slice(block_ref.timestamp.to_be_bytes().as_ref());

        // Send a hint for the blob commitment and field elements.
        HintType::L1Blob.with_data(&[blob_req_meta.as_ref()]).send(self.oracle.as_ref()).await?;

        // Fetch the blob commitment.
        let mut commitment = [0u8; 48];
        self.oracle
            .get_exact(PreimageKey::new(*blob_hash.hash, PreimageKeyType::Sha256), &mut commitment)
            .await
            .map_err(OracleProviderError::Preimage)?;

        // Reconstruct the blob from the 4096 field elements.
        let mut blob = Blob::default();
        let mut field_element_key = [0u8; 80];
        field_element_key[..48].copy_from_slice(commitment.as_ref());
        for i in 0..FIELD_ELEMENTS_PER_BLOB {
            field_element_key[72..].copy_from_slice(i.to_be_bytes().as_ref());

            let mut field_element = [0u8; 32];
            self.oracle
                .get_exact(
                    PreimageKey::new(*keccak256(field_element_key), PreimageKeyType::Blob),
                    &mut field_element,
                )
                .await
                .map_err(OracleProviderError::Preimage)?;
            blob[(i as usize) << 5..(i as usize + 1) << 5].copy_from_slice(field_element.as_ref());
        }

        tracing::info!(target: "client_oracle", "Retrieved blob {blob_hash:?} from the oracle.");

        Ok(blob)
    }
}

#[async_trait]
impl<T: CommsClient + Sync + Send> BlobProvider for OracleBlobProvider<T> {
    type Error = OracleProviderError;

    async fn get_blobs(
        &mut self,
        block_ref: &BlockInfo,
        blob_hashes: &[IndexedBlobHash],
    ) -> Result<Vec<Box<Blob>>, Self::Error> {
        let mut blobs = Vec::with_capacity(blob_hashes.len());
        for hash in blob_hashes {
            blobs.push(Box::new(self.get_blob(block_ref, hash).await?));
        }
        Ok(blobs)
    }
}
