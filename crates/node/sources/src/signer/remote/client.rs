use alloy_primitives::SignatureError;
use alloy_rpc_client::ClientBuilder;
use alloy_transport_http::Http;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use url::Url;

use crate::{
    RemoteSignerHandler,
    signer::remote::cert::{CertificateError, ClientCert},
};

/// Configuration for the remote signer client
///
/// This configuration supports various TLS/certificate scenarios:
///
/// 1. **Basic HTTPS**: Only `endpoint` and `address` are required.
/// 2. **Custom CA**: Provide `ca_cert` to verify servers with custom/self-signed certificates.
/// 3. **Mutual TLS (mTLS)**: Provide both `client_cert` and `client_key` for client authentication.
/// 4. **Full mTLS with custom CA**: Combine all certificate options for maximum security.
///
/// Certificate formats supported:
/// - PEM format for all certificates and keys
/// - Certificates should be provided as file paths.
///
/// By default, the process will watch for changes in the client certificate files and reload the
/// client automatically.
#[derive(Debug, Clone)]
pub struct RemoteSigner {
    /// The URL of the remote signer endpoint
    pub endpoint: Url,
    /// Optional client certificate for mTLS (PEM format)
    pub client_cert: Option<ClientCert>,
    /// Optional CA certificate for server verification (PEM format)
    pub ca_cert: Option<std::path::PathBuf>,
    /// Request timeout in seconds
    pub timeout_secs: Option<u64>,
}

impl RemoteSigner {
    /// Creates a new remote signer with the given configuration
    ///
    /// If client certificates are configured, this will automatically start a certificate watcher
    /// that monitors the certificate files for changes. When certificates are updated (e.g., by
    /// cert-manager in Kubernetes), the TLS client will be automatically reloaded with the new
    /// certificates without requiring a restart.
    ///
    /// # Certificate Watching
    ///
    /// The certificate watcher monitors:
    /// - Client certificate file (if mTLS is configured)
    /// - Client private key file (if mTLS is configured)
    /// - CA certificate file (if custom CA is configured)
    ///
    /// When any of these files are modified, the watcher will:
    /// 1. Log the certificate change event
    /// 2. Reload the certificate files from disk
    /// 3. Rebuild the HTTP client with the new TLS configuration
    /// 4. Replace the existing client atomically
    ///
    /// This enables zero-downtime certificate rotation in production environments.
    pub async fn start(self) -> Result<RemoteSignerHandler, RemoteSignerError> {
        let http_client = self.build_http_client()?;
        let transport = Http::with_client(http_client, self.endpoint.clone());
        let client = ClientBuilder::default().transport(transport, true);

        // Try to ping the signer to check if it's reachable
        let version: String =
            client.request("health_status", ()).await.map_err(RemoteSignerError::PingError)?;

        tracing::info!(target: "signer", version, "Connected to op-signer server");

        let client = Arc::new(RwLock::new(client));

        // Start certificate watcher if client certificates are configured
        let watcher_handle = self.start_certificate_watcher(client.clone()).await?;

        Ok(RemoteSignerHandler { client, watcher_handle })
    }

    /// Builds an HTTP client with certificate handling for the remote signer
    pub(super) fn build_http_client(&self) -> Result<reqwest::Client, RemoteSignerError> {
        let mut client_builder = reqwest::Client::builder();

        // Set timeout if specified
        if let Some(timeout_secs) = self.timeout_secs {
            client_builder = client_builder.timeout(std::time::Duration::from_secs(timeout_secs));
        }

        // Configure TLS if certificates are provided
        if self.client_cert.is_some() || self.ca_cert.is_some() {
            let tls_config = self.build_tls_config()?;
            client_builder = client_builder.use_preconfigured_tls(tls_config);
        }

        client_builder.build().map_err(RemoteSignerError::BuildError)
    }
}

/// Errors that can occur when using the remote signer
#[derive(Debug, Error)]
pub enum RemoteSignerError {
    /// JSON-RPC transport error
    #[error("JSON-RPC transport error: {0}")]
    SigningRPCError(#[from] alloy_transport::TransportError),
    /// JSON serialization error
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// HTTP client build error
    #[error("HTTP client build error: {0}")]
    BuildError(#[from] reqwest::Error),
    /// Failed to ping signer
    #[error("Failed to ping signer: {0}")]
    PingError(alloy_transport::TransportError),
    /// Invalid certificate error
    #[error("Invalid certificate: {0}")]
    CertificateError(#[from] CertificateError),
    /// Certificate watcher error
    #[error("Certificate watcher error: {0}")]
    CertificateWatcherError(#[from] notify::Error),
    /// Invalid signature hex encoding
    #[error("Invalid signature hex encoding: {0}")]
    InvalidSignatureHex(hex::FromHexError),
    /// Invalid signature length
    #[error("Invalid signature length, expected 65 bytes, got {0}")]
    InvalidSignatureLength(usize),
    /// Signature error
    #[error("Signature error: {0}")]
    SignatureError(#[from] SignatureError),
}
