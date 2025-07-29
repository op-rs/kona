use std::path::PathBuf;

use alloy_primitives::{Address, B256};
use alloy_signer::{Signer, k256::ecdsa};
use alloy_signer_local::PrivateKeySigner;
use clap::{Parser, arg};
use kona_sources::{BlockSigner, ClientCert, RemoteSigner};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::str::FromStr;
use url::Url;

use crate::flags::GlobalArgs;

/// Signer CLI Flags
#[derive(Debug, Clone, Parser, Default, PartialEq, Eq)]
pub struct SignerArgs {
    /// An optional flag to specify a local private key for the sequencer to sign unsafe blocks.
    #[arg(
        long = "p2p.sequencer.key",
        env = "KONA_NODE_P2P_SEQUENCER_KEY",
        conflicts_with = "endpoint"
    )]
    pub sequencer_key: Option<B256>,
    /// The URL of the remote signer endpoint. If not provided, remote signer will be disabled.
    /// This is mutually exclusive with `p2p.sequencer.key`.
    /// This is required if any of the other signer flags are provided.
    #[arg(
        long = "p2p.signer.endpoint",
        env = "KONA_NODE_P2P_SIGNER_ENDPOINT",
        requires = "address"
    )]
    pub endpoint: Option<Url>,
    /// The address to sign transactions for. Required if `signer.endpoint` is provided.
    #[arg(
        long = "p2p.signer.address",
        env = "KONA_NODE_P2P_SIGNER_ADDRESS",
        requires = "endpoint"
    )]
    pub address: Option<Address>,
    /// Headers to pass to the remote signer. Format `key=value`. Value can contain any character
    /// allowed in a HTTP header. When using env vars, split with commas. When using flags one
    /// key value pair per flag.
    #[arg(long = "p2p.signer.header", env = "KONA_NODE_P2P_SIGNER_HEADER", requires = "endpoint")]
    pub header: Vec<String>,
    /// An optional path to CA certificates to be used for the remote signer.
    #[arg(long = "p2p.signer.tls.ca", env = "KONA_NODE_P2P_SIGNER_TLS_CA", requires = "endpoint")]
    pub ca_cert: Option<PathBuf>,
    /// An optional path to the client certificate for the remote signer. If specified,
    /// `signer.tls.key` must also be specified.
    #[arg(
        long = "p2p.signer.tls.cert",
        env = "KONA_NODE_P2P_SIGNER_TLS_CERT",
        requires = "key",
        requires = "endpoint"
    )]
    pub cert: Option<PathBuf>,
    /// An optional path to the client key for the remote signer. If specified,
    /// `signer.tls.cert` must also be specified.
    #[arg(
        long = "p2p.signer.tls.key",
        env = "KONA_NODE_P2P_SIGNER_TLS_KEY",
        requires = "cert",
        requires = "endpoint"
    )]
    pub key: Option<PathBuf>,
}

/// Errors that can occur when parsing the signer arguments.
#[derive(Debug, thiserror::Error)]
pub enum SignerArgsParseError {
    /// The local sequencer key and remote signer cannot be specified at the same time.
    #[error("A local sequencer key and a remote signer cannot be specified at the same time.")]
    LocalAndRemoteSigner,
    /// The sequencer key is invalid.
    #[error("The sequencer key is invalid.")]
    SequencerKeyInvalid(#[from] ecdsa::Error),
    /// The address is required if `signer.endpoint` is provided.
    #[error("The address is required if `signer.endpoint` is provided.")]
    AddressRequired,
    /// The header is invalid.
    #[error("The header is invalid.")]
    InvalidHeader,
    /// The private key field is required if `signer.tls.cert` is provided.
    #[error("The private key field is required if `signer.tls.cert` is provided.")]
    KeyRequired,
    /// The header name is invalid.
    #[error("The header name is invalid.")]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),
    /// The header value is invalid.
    #[error("The header value is invalid.")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
}

impl SignerArgs {
    /// Creates a [`BlockSigner`] from the [`SignerArgs`].
    pub fn config(self, args: &GlobalArgs) -> Result<Option<BlockSigner>, SignerArgsParseError> {
        // The sequencer signer obtained from the CLI arguments.
        let gossip_signer: Option<BlockSigner> = match (self.sequencer_key, self.config_remote()?) {
            (Some(_), Some(_)) => return Err(SignerArgsParseError::LocalAndRemoteSigner),
            (Some(key), None) => {
                let signer: BlockSigner = PrivateKeySigner::from_bytes(&key)?
                    .with_chain_id(Some(args.l2_chain_id.into()))
                    .into();
                Some(signer)
            }
            (None, Some(signer)) => Some(signer.into()),
            (None, None) => None,
        };

        Ok(gossip_signer)
    }

    /// Creates a [`RemoteSigner`] from the [`SignerArgs`].
    fn config_remote(self) -> Result<Option<RemoteSigner>, SignerArgsParseError> {
        let Some(endpoint) = self.endpoint else {
            return Ok(None);
        };

        let Some(address) = self.address else {
            return Err(SignerArgsParseError::AddressRequired);
        };

        let headers = self
            .header
            .iter()
            .map(|h| {
                let (key, value) = h.split_once('=').ok_or(SignerArgsParseError::InvalidHeader)?;
                Ok((HeaderName::from_str(key)?, HeaderValue::from_str(value)?))
            })
            .collect::<Result<HeaderMap, SignerArgsParseError>>()?;

        let client_cert = self
            .cert
            .clone()
            .map(|cert| {
                Ok::<_, SignerArgsParseError>(ClientCert {
                    cert,
                    key: self.key.clone().ok_or(SignerArgsParseError::KeyRequired)?,
                })
            })
            .transpose()?;

        Ok(Some(RemoteSigner {
            address,
            endpoint,
            ca_cert: self.ca_cert.clone(),
            client_cert,
            headers,
        }))
    }
}
