use std::{fmt::Display, time::Duration};

use alloy_primitives::hex;
use async_trait::async_trait;
use kona_derive::errors::{PipelineError, PipelineErrorKind};
use reqwest::{Client, Response};

/// An error for the [DaProvider].
#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum DaProviderError {
    #[error("upload image error: {0}")]
    ErrDAProxyUpload(String),

    #[error("download image error: {0}")]
    ErrDAProxyDownload(String),

    #[error("response data format error: {0}")]
    ErrDAProxyResponse(String),
}

impl From<DaProviderError> for PipelineErrorKind {
    fn from(e: DaProviderError) -> Self {
        match e {
            DaProviderError::ErrDAProxyUpload(e) => PipelineErrorKind::Temporary(
                PipelineError::Provider(format!("da proxy error: {e}")),
            ),
            DaProviderError::ErrDAProxyDownload(e) => PipelineErrorKind::Temporary(
                PipelineError::Provider(format!("da proxy error: {e}")),
            ),
            DaProviderError::ErrDAProxyResponse(e) => PipelineErrorKind::Temporary(
                PipelineError::Provider(format!("da proxy error: {e}")),
            )
        }
    }
}

/// The DaProvider trait specifies the functionality of a data source that can provide blobs.
#[async_trait]
pub trait DaProvider {
    /// The error type for the [DaProvider].
    type Error: Display + ToString + Into<PipelineErrorKind>;
    
    /// submit data to da provider, return data can be used as data id for later fetch operation
    async fn submit_data(&self, data: Vec<u8>) -> Result<Vec<u8>, Self::Error>;
    /// fetch data by key from DA provider
    async fn fetch_data(&self, key: Vec<u8>) -> Result<Vec<u8>, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct OnlineDaProvider {
    /// The base URL of the DA provider.
    pub end_point: String,
    /// The inner reqwest client.
    pub inner: Client,
}

impl OnlineDaProvider {
    /// Creates a new [OnlineDaProvider] with the given base URL.
    pub fn new(end_point: String) -> Self {
        Self { end_point, inner: Client::new() }
    }

    async fn parse_response(&self, res: Response) -> Result<Vec<u8>, DaProviderError> {
        if res.status().is_success() {
            Ok(Vec::from(res.bytes().await.map_err(|err| {
                DaProviderError::ErrDAProxyResponse(err.to_string())
            })?))
        } else {
            Err(DaProviderError::ErrDAProxyResponse(format!(
                "receive status code:{}",
                res.status()
            )))
        }
    }

    pub async fn set_input(&self, data: Vec<u8>) -> Result<Vec<u8>, DaProviderError> {
        let res = self.inner
        .post(format!("{}put/", self.end_point))
        .header("Content-Type", "application/octet-stream")
        .body(data)
        .timeout(Duration::from_secs(900))
        .send()
        .await
        .map_err(|err| DaProviderError::ErrDAProxyUpload(err.to_string()))?;

        Ok(self.parse_response(res).await?)
    }

    pub async fn get_input(&self, key: Vec<u8>) -> Result<Vec<u8>, DaProviderError> {
        let res = self.inner
            .get(format!(
                "{}get/0x{}",
                self.end_point,
                hex::encode(key)
            ))
            .timeout(Duration::from_secs(60))
            .send()
            .await
            .map_err(|err| DaProviderError::ErrDAProxyDownload(err.to_string()))?;

        Ok(self.parse_response(res).await?)
    }
}

#[async_trait]
impl DaProvider for OnlineDaProvider {
    type Error = DaProviderError;

    async fn submit_data(&self, data: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        self.set_input(data).await
    }

    async fn fetch_data(&self, key: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        self.get_input(key).await
    }
}
