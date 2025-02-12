//! Contains an online implementation of the `BeaconClient` trait.

use alloy_eips::eip4844::IndexedBlobHash;
use alloy_rpc_types_beacon::sidecar::{BeaconBlobBundle, BlobData};
use async_trait::async_trait;
use reqwest::Client;
use std::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

/// The config spec engine api method.
const SPEC_METHOD: &str = "eth/v1/config/spec";

/// The beacon genesis engine api method.
const GENESIS_METHOD: &str = "eth/v1/beacon/genesis";

/// The blob sidecars engine api method prefix.
const SIDECARS_METHOD_PREFIX: &str = "eth/v1/beacon/blob_sidecars";

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
    type Error: core::fmt::Display + ToString;

    /// Returns the config spec.
    async fn config_spec(&self) -> Result<APIConfigResponse, Self::Error>;

    /// Returns the beacon genesis.
    async fn beacon_genesis(&self) -> Result<APIGenesisResponse, Self::Error>;

    /// Fetches blob sidecars that were confirmed in the specified L1 block with the given indexed
    /// hashes. Order of the returned sidecars is guaranteed to be that of the hashes. Blob data is
    /// not checked for validity.
    async fn beacon_blob_side_cars(
        &self,
        slot: u64,
        hashes: &[IndexedBlobHash],
    ) -> Result<Vec<BlobData>, Self::Error>;
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

#[async_trait]
impl BeaconClient for OnlineBeaconClient {
    type Error = reqwest::Error;

    async fn config_spec(&self) -> Result<APIConfigResponse, Self::Error> {
        let first = self.inner.get(format!("{}/{}", self.base, SPEC_METHOD)).send().await?;
        first.json::<APIConfigResponse>().await
    }

    async fn beacon_genesis(&self) -> Result<APIGenesisResponse, Self::Error> {
        let first = self.inner.get(format!("{}/{}", self.base, GENESIS_METHOD)).send().await?;
        first.json::<APIGenesisResponse>().await
    }

    async fn beacon_blob_side_cars(
        &self,
        slot: u64,
        hashes: &[IndexedBlobHash],
    ) -> Result<Vec<BlobData>, Self::Error> {
        let raw_response = self
            .inner
            .get(format!("{}/{}/{}", self.base, SIDECARS_METHOD_PREFIX, slot))
            .send()
            .await?;
        let raw_response = raw_response.json::<BeaconBlobBundle>().await?;

        // Filter the sidecars by the hashes, in-order.
        let mut sidecars = Vec::with_capacity(hashes.len());
        hashes.iter().for_each(|hash| {
            if let Some(sidecar) =
                raw_response.data.iter().find(|sidecar| sidecar.index == hash.index)
            {
                sidecars.push(sidecar.clone());
            }
        });

        Ok(sidecars)
    }
}
