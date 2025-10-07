//! Contains an online implementation of the `BeaconClient` trait.

#[cfg(feature = "metrics")]
use crate::Metrics;
use alloy_consensus::Blob;
use alloy_eips::eip4844::{IndexedBlobHash, deserialize_blob};
use alloy_rpc_types_beacon::sidecar::BeaconBlobBundle;
use async_trait::async_trait;
use reqwest::Client;
use std::{boxed::Box, format, ops::Deref, string::String, vec::Vec};

/// The config spec engine api method.
const SPEC_METHOD: &str = "eth/v1/config/spec";

/// The beacon genesis engine api method.
const GENESIS_METHOD: &str = "eth/v1/beacon/genesis";

/// The blob sidecars engine api method prefix.
const SIDECARS_METHOD_PREFIX_DEPRECATED: &str = "eth/v1/beacon/blob_sidecars";

/// A reduced genesis data.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReducedGenesisData {
    /// The genesis time.
    #[serde(rename = "genesis_time")]
    #[serde(with = "alloy_serde::quantity")]
    pub genesis_time: u64,
}

/// An API genesis response.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct APIGenesisResponse {
    /// The data.
    pub data: ReducedGenesisData,
}

/// A reduced config data.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReducedConfigData {
    /// The seconds per slot.
    #[serde(rename = "SECONDS_PER_SLOT")]
    #[serde(with = "alloy_serde::quantity")]
    pub seconds_per_slot: u64,
}

/// An API config response.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct APIConfigResponse {
    /// The data.
    pub data: ReducedConfigData,
}

impl APIConfigResponse {
    /// Creates a new API config response.
    pub const fn new(seconds_per_slot: u64) -> Self {
        Self { data: ReducedConfigData { seconds_per_slot } }
    }
}

impl APIGenesisResponse {
    /// Creates a new API genesis response.
    pub const fn new(genesis_time: u64) -> Self {
        Self { data: ReducedGenesisData { genesis_time } }
    }
}

/// The [BeaconClient] is a thin wrapper around the Beacon API.
#[async_trait]
pub trait BeaconClient {
    /// The error type for [BeaconClient] implementations.
    type Error: core::fmt::Display;

    /// Returns the config spec.
    async fn config_spec(&self) -> Result<APIConfigResponse, Self::Error>;

    /// Returns the beacon genesis.
    async fn beacon_genesis(&self) -> Result<APIGenesisResponse, Self::Error>;

    /// Fetches blobs that were confirmed in the specified L1 block with the given slot.
    /// Blob data is not checked for validity.
    async fn filtered_beacon_blobs(
        &self,
        slot: u64,
        blob_hashes: &[IndexedBlobHash],
    ) -> Result<Vec<BoxedBlobWithIndex>, Self::Error>;
}

/// An online implementation of the [BeaconClient] trait.
#[derive(Debug, Clone)]
pub struct OnlineBeaconClient {
    /// The base URL of the beacon API.
    pub base: String,
    /// The inner reqwest client.
    pub inner: Client,
}

impl OnlineBeaconClient {
    /// Creates a new [OnlineBeaconClient] from the provided [reqwest::Url].
    pub fn new_http(mut base: String) -> Self {
        // If base ends with a slash, remove it
        if base.ends_with("/") {
            base.remove(base.len() - 1);
        }
        Self { base, inner: Client::new() }
    }
}

/// A boxed blob. This is used to deserialize the blobs endpoint response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BoxedBlob {
    /// The blob data.
    #[serde(deserialize_with = "deserialize_blob")]
    pub blob: Box<Blob>,
}

/// A boxed blob with index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxedBlobWithIndex {
    /// The index of the blob.
    pub index: u64,
    /// The blob data.
    pub blob: Box<Blob>,
}

impl Deref for BoxedBlob {
    type Target = Blob;

    fn deref(&self) -> &Self::Target {
        &self.blob
    }
}

/// A blobs bundle. This is used to deserialize the blobs endpoint response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct BlobsBundle {
    pub data: Vec<BoxedBlob>,
}

impl From<BeaconBlobBundle> for BlobsBundle {
    fn from(value: BeaconBlobBundle) -> Self {
        let blobs = value.data.into_iter().map(|blob| BoxedBlob { blob: blob.blob }).collect();
        Self { data: blobs }
    }
}

#[async_trait]
impl BeaconClient for OnlineBeaconClient {
    type Error = reqwest::Error;

    async fn config_spec(&self) -> Result<APIConfigResponse, Self::Error> {
        kona_macros::inc!(gauge, Metrics::BEACON_CLIENT_REQUESTS, "method" => "spec");

        let result = async {
            let first = self.inner.get(format!("{}/{}", self.base, SPEC_METHOD)).send().await?;
            first.json::<APIConfigResponse>().await
        }
        .await;

        #[cfg(feature = "metrics")]
        if result.is_err() {
            kona_macros::inc!(gauge, Metrics::BEACON_CLIENT_ERRORS, "method" => "spec");
        }

        result
    }

    async fn beacon_genesis(&self) -> Result<APIGenesisResponse, Self::Error> {
        kona_macros::inc!(gauge, Metrics::BEACON_CLIENT_REQUESTS, "method" => "genesis");

        let result = async {
            let first = self.inner.get(format!("{}/{}", self.base, GENESIS_METHOD)).send().await?;
            first.json::<APIGenesisResponse>().await
        }
        .await;

        #[cfg(feature = "metrics")]
        if result.is_err() {
            kona_macros::inc!(gauge, Metrics::BEACON_CLIENT_ERRORS, "method" => "genesis");
        }

        result
    }

    async fn filtered_beacon_blobs(
        &self,
        slot: u64,
        blob_hashes: &[IndexedBlobHash],
    ) -> Result<Vec<BoxedBlobWithIndex>, Self::Error> {
        kona_macros::inc!(gauge, Metrics::BEACON_CLIENT_REQUESTS, "method" => "blob_sidecars");

        let blob_indexes = blob_hashes.iter().map(|blob| blob.index).collect::<Vec<_>>();

        let raw_response = self
            .inner
            .get(format!("{}/{}/{}", self.base, SIDECARS_METHOD_PREFIX_DEPRECATED, slot))
            .send()
            .await?;

        Ok(raw_response
            .json::<BeaconBlobBundle>()
            .await?
            .into_iter()
            .filter_map(|blob| {
                blob_indexes
                    .contains(&blob.index)
                    .then_some(BoxedBlobWithIndex { index: blob.index, blob: blob.blob })
            })
            .collect::<Vec<_>>())
    }
}
