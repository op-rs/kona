use std::{io::BufReader, sync::Arc};

use alloy_rpc_client::{ClientBuilder, RpcClient};
use alloy_transport_http::Http;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rustls::{ClientConfig, RootCertStore};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::signer::remote::client::RemoteSigner;

/// Client certificate and key pair for mTLS authentication (PEM format)
#[derive(Debug, Clone)]
pub struct ClientCert {
    /// Path to the client certificate in PEM format
    pub cert: std::path::PathBuf,
    /// Path to the client private key in PEM format
    pub key: std::path::PathBuf,
}

/// Errors that can occur when handling certificates
#[derive(Debug, Error)]
pub enum CertificateError {
    /// Invalid CA certificate path
    #[error("Invalid CA certificate path: {0}")]
    InvalidCACertificatePath(std::io::Error),
    /// Invalid certificate error
    #[error("Invalid CA certificate: {0}")]
    InvalidCACertificate(std::io::Error),
    /// Failed to add CA certificate
    #[error("Failed to add CA certificate: {0}")]
    AddCACertificate(rustls::Error),
    /// Failed to configure client auth
    #[error("Failed to configure client auth: {0}")]
    ConfigureClientAuth(rustls::Error),
    /// Invalid client certificate path
    #[error("Invalid client certificate path: {0}")]
    InvalidClientCertificatePath(std::io::Error),
    /// Invalid client certificate
    #[error("Invalid client certificate: {0}")]
    InvalidClientCertificate(std::io::Error),
    /// Invalid private key path
    #[error("Invalid private key path: {0}")]
    InvalidPrivateKeyPath(std::io::Error),
    /// Invalid private key
    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(std::io::Error),
    /// No private key found while parsing the client certificate
    #[error("No private key found while parsing the client certificate")]
    NoPrivateKey,
}

impl RemoteSigner {
    /// Builds TLS configuration with certificate handling for the remote signer
    pub(super) fn build_tls_config(&self) -> Result<ClientConfig, CertificateError> {
        let mut root_store = RootCertStore::empty();

        // Add custom CA certificate if provided
        if let Some(ca_cert_path) = &self.ca_cert {
            let ca_cert_file = std::fs::File::open(ca_cert_path)
                .map_err(CertificateError::InvalidCACertificatePath)?;
            let mut ca_cert_reader = BufReader::new(ca_cert_file);
            let ca_cert = rustls_pemfile::certs(&mut ca_cert_reader)
                .collect::<Result<Vec<_>, _>>()
                .map_err(CertificateError::InvalidCACertificate)?;

            for cert in ca_cert {
                root_store.add(cert).map_err(CertificateError::AddCACertificate)?;
            }
        }

        let tls_config = ClientConfig::builder().with_root_certificates(root_store);

        // Configure client certificates for mTLS if provided
        match &self.client_cert {
            None => Ok(tls_config.with_no_client_auth()),
            Some(ClientCert { cert, key }) => {
                let cert_file = std::fs::File::open(cert)
                    .map_err(CertificateError::InvalidClientCertificatePath)?;
                let mut cert_reader = BufReader::new(cert_file);
                let certs = rustls_pemfile::certs(&mut cert_reader)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CertificateError::InvalidClientCertificate)?;

                let key_file =
                    std::fs::File::open(key).map_err(CertificateError::InvalidPrivateKeyPath)?;
                let mut key_reader = BufReader::new(key_file);
                let key = rustls_pemfile::private_key(&mut key_reader)
                    .map_err(CertificateError::InvalidPrivateKey)?
                    .ok_or_else(|| CertificateError::NoPrivateKey)?;

                Ok(tls_config
                    .with_client_auth_cert(certs, key)
                    .map_err(CertificateError::ConfigureClientAuth)?)
            }
        }
    }

    /// Starts a certificate watcher that monitors client certificate files and reloads the client
    /// automatically when they are updated.
    ///
    /// Returns `Ok(None)` if no client certificates are configured.
    pub(super) async fn start_certificate_watcher(
        &self,
        client: Arc<RwLock<RpcClient>>,
    ) -> Result<Option<RecommendedWatcher>, notify::Error> {
        let Some(ref client_cert) = self.client_cert.clone() else {
            return Ok(None);
        };

        // Clone the builder to avoid borrowing issues
        let builder = self.clone();
        let mut watcher = notify::recommended_watcher(move |res| {
            builder.handle_watcher_event(client.clone(), res)
        })?;

        tracing::info!(target: "signer", "Starting certificate watcher for automatic TLS reload");

        watcher.watch(&client_cert.cert, RecursiveMode::NonRecursive)?;
        watcher.watch(&client_cert.key, RecursiveMode::NonRecursive)?;

        Ok(Some(watcher))
    }

    /// Handles certificate watcher events
    ///
    /// This function is called by the certificate watcher when a certificate file is modified.
    /// It reloads the TLS configuration and updates the client.
    fn handle_watcher_event(
        &self,
        client: Arc<RwLock<RpcClient>>,
        res: Result<Event, notify::Error>,
    ) {
        match res {
            Ok(Event { kind: EventKind::Modify(_), .. }) => {
                tracing::debug!(
                    target: "signer:certificate-watcher",
                    "Certificate file changed, reloading TLS configuration"
                );

                match self.build_http_client() {
                    Ok(new_client) => {
                        let transport = Http::with_client(new_client, self.endpoint.clone());
                        let new_client = ClientBuilder::default().transport(transport, false);

                        // Update the client with the new TLS configuration. We're using a blocking
                        // write here because the handler is synchronous.
                        let mut client_guard = client.blocking_write();
                        *client_guard = new_client;
                        tracing::info!(target: "signer:certificate-watcher", "TLS configuration reloaded successfully");
                    }
                    Err(e) => {
                        tracing::error!(target: "signer:certificate-watcher", error = %e, "Failed to reload TLS configuration");
                    }
                }
            }
            Ok(event) => {
                tracing::trace!(target: "signer:certificate-watcher", event = ?event, "Ignoring non-modify event.");
            }
            Err(e) => {
                tracing::error!(target: "signer:certificate-watcher", error = %e, "Failed to receive event from watcher channel.");
            }
        }
    }
}
