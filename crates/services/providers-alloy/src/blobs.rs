//! Contains an online implementation of the `BlobProvider` trait.

use crate::BeaconClient;
use alloy_eips::eip4844::{Blob, BlobTransactionSidecarItem, IndexedBlobHash};
use alloy_rpc_types_beacon::sidecar::BlobData;
use async_trait::async_trait;
use kona_derive::{errors::BlobProviderError, traits::BlobProvider};
use maili_protocol::BlockInfo;
use std::{boxed::Box, string::ToString, vec::Vec};

/// An online implementation of the [BlobProvider] trait.
#[derive(Debug, Clone)]
pub struct OnlineBlobProvider<B: BeaconClient> {
    /// The Beacon API client.
    pub beacon_client: B,
    /// Beacon Genesis time used for the time to slot conversion.
    pub genesis_time: u64,
    /// Slot interval used for the time to slot conversion.
    pub slot_interval: u64,
}

impl<B: BeaconClient> OnlineBlobProvider<B> {
    /// Creates a new instance of the [OnlineBlobProvider].
    ///
    /// The `genesis_time` and `slot_interval` arguments are _optional_ and the
    /// [OnlineBlobProvider] will attempt to load them dynamically at runtime if they are not
    /// provided.
    ///
    /// ## Panics
    /// Panics if the genesis time or slot interval cannot be loaded from the beacon client.
    pub async fn init(beacon_client: B) -> Self {
        let genesis_time = beacon_client
            .beacon_genesis()
            .await
            .map(|r| r.data.genesis_time)
            .map_err(|e| BlobProviderError::Backend(e.to_string()))
            .expect("Failed to load genesis time from beacon client");
        let slot_interval = beacon_client
            .config_spec()
            .await
            .map(|r| r.data.seconds_per_slot)
            .map_err(|e| BlobProviderError::Backend(e.to_string()))
            .expect("Failed to load slot interval from beacon client");
        Self { beacon_client, genesis_time, slot_interval }
    }

    /// Fetches blob sidecars for the given slot and blob hashes.
    pub async fn fetch_sidecars(
        &self,
        slot: u64,
        hashes: &[IndexedBlobHash],
    ) -> Result<Vec<BlobData>, BlobProviderError> {
        self.beacon_client
            .beacon_blob_side_cars(slot, hashes)
            .await
            .map_err(|e| BlobProviderError::Backend(e.to_string()))
    }

    /// Computes the slot for the given timestamp.
    pub const fn slot(
        genesis: u64,
        slot_time: u64,
        timestamp: u64,
    ) -> Result<u64, BlobProviderError> {
        if timestamp < genesis {
            return Err(BlobProviderError::SlotDerivation);
        }
        Ok((timestamp - genesis) / slot_time)
    }

    /// Fetches blob sidecars for the given block reference and blob hashes.
    pub async fn fetch_filtered_sidecars(
        &self,
        block_ref: &BlockInfo,
        blob_hashes: &[IndexedBlobHash],
    ) -> Result<Vec<BlobTransactionSidecarItem>, BlobProviderError> {
        if blob_hashes.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate the slot for the given timestamp.
        let slot = Self::slot(self.genesis_time, self.slot_interval, block_ref.timestamp)?;

        // Fetch blob sidecars for the slot using the given blob hashes.
        let sidecars = self.fetch_sidecars(slot, blob_hashes).await?;

        // Filter blob sidecars that match the indicies in the specified list.
        let blob_hash_indicies = blob_hashes.iter().map(|b| b.index).collect::<Vec<u64>>();
        let filtered = sidecars
            .into_iter()
            .filter(|s| blob_hash_indicies.contains(&s.index))
            .collect::<Vec<_>>();

        // Validate the correct number of blob sidecars were retrieved.
        if blob_hashes.len() != filtered.len() {
            return Err(BlobProviderError::SidecarLengthMismatch(blob_hashes.len(), filtered.len()));
        }

        Ok(filtered
            .into_iter()
            .map(|bs| BlobTransactionSidecarItem {
                index: bs.index,
                blob: bs.blob,
                kzg_commitment: bs.kzg_commitment,
                kzg_proof: bs.kzg_proof,
            })
            .collect::<Vec<BlobTransactionSidecarItem>>())
    }
}

#[async_trait]
impl<B> BlobProvider for OnlineBlobProvider<B>
where
    B: BeaconClient + Send + Sync,
{
    type Error = BlobProviderError;

    /// Fetches blob sidecars that were confirmed in the specified L1 block with the given indexed
    /// hashes. The blobs are validated for their index and hashes using the specified
    /// [IndexedBlobHash].
    async fn get_blobs(
        &mut self,
        block_ref: &BlockInfo,
        blob_hashes: &[IndexedBlobHash],
    ) -> Result<Vec<Box<Blob>>, Self::Error> {
        // Fetch the blob sidecars for the given block reference and blob hashes.
        let sidecars = self.fetch_filtered_sidecars(block_ref, blob_hashes).await?;

        // Validate the blob sidecars straight away with the num hashes.
        let blobs = sidecars
            .into_iter()
            .enumerate()
            .map(|(i, sidecar)| {
                let hash = blob_hashes
                    .get(i)
                    .ok_or(BlobProviderError::Backend("Missing blob hash".to_string()))?;
                sidecar
                    .verify_blob(&IndexedBlobHash { hash: hash.hash, index: hash.index })
                    .map(|_| sidecar.blob)
                    .map_err(|e| BlobProviderError::Backend(e.to_string()))
            })
            .collect::<Result<Vec<Box<Blob>>, BlobProviderError>>()
            .map_err(|e| BlobProviderError::Backend(e.to_string()))?;
        Ok(blobs)
    }
}

/// The minimal interface required to fetch sidecars from a remote blob store.
#[async_trait]
pub trait BlobSidecarProvider {
    /// Fetches blob sidecars that were confirmed in the specified L1 block with the given indexed
    /// hashes. Order of the returned sidecars is guaranteed to be that of the hashes. Blob data is
    /// not checked for validity.
    ///
    /// Consensus specs: <https://ethereum.github.io/beacon-APIs/#/Beacon/getBlobSidecars>
    async fn beacon_blob_side_cars(
        &self,
        slot: u64,
        hashes: &[IndexedBlobHash],
    ) -> Result<Vec<BlobData>, BlobProviderError>;
}

/// Blanket implementation of the [BlobSidecarProvider] trait for all types that
/// implemend [BeaconClient], which has a superset of the required functionality.
#[async_trait]
impl<B: BeaconClient + Send + Sync> BlobSidecarProvider for B {
    async fn beacon_blob_side_cars(
        &self,
        slot: u64,
        hashes: &[IndexedBlobHash],
    ) -> Result<Vec<BlobData>, BlobProviderError> {
        self.beacon_blob_side_cars(slot, hashes)
            .await
            .map_err(|e| BlobProviderError::Backend(e.to_string()))
    }
}
