use std::sync::Arc;

use alloy_primitives::{Address, B256, ChainId};
use alloy_rpc_client::RpcClient;
use alloy_signer::Signature;
use notify::INotifyWatcher;
use op_alloy_rpc_types_engine::PayloadHash;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::RemoteSignerError;

/// Request parameters for signing a block payload
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BlockPayloadArgs {
    domain: B256,
    chain_id: u64,
    payload_hash: B256,
    sender_address: Address,
}

/// Response from the remote signer
#[derive(Debug, Deserialize)]
struct SignResponse {
    signature: String,
}

/// Remote signer that communicates with an external signing service via JSON-RPC
#[derive(Debug)]
pub struct RemoteSignerHandler {
    pub(super) client: Arc<RwLock<RpcClient>>,
    pub(super) watcher_handle: Option<INotifyWatcher>,
}

impl RemoteSignerHandler {
    /// Returns true if certificate watching is enabled
    pub const fn is_certificate_watching_enabled(&self) -> bool {
        self.watcher_handle.is_some()
    }

    /// Signs a block payload hash using the remote signer via JSON-RPC
    pub async fn sign_block_v1(
        &self,
        payload_hash: PayloadHash,
        chain_id: ChainId,
        sender_address: Address,
    ) -> Result<Signature, RemoteSignerError> {
        let params = BlockPayloadArgs {
            // For v1 payloads, the domain is always zero
            domain: B256::ZERO,
            chain_id,
            payload_hash: payload_hash.0,
            sender_address,
        };

        // Make JSON-RPC call to the custom method
        let response: SignResponse = {
            self.client
                .read()
                .await
                .request("opsigner_signBlockPayload", &params)
                .await
                .map_err(RemoteSignerError::SigningRPCError)?
        };

        // Parse the hex signature
        let signature_bytes = hex::decode(response.signature.trim_start_matches("0x"))
            .map_err(RemoteSignerError::InvalidSignatureHex)?;

        if signature_bytes.len() != 65 {
            return Err(RemoteSignerError::InvalidSignatureLength(signature_bytes.len()));
        }

        let signature = Signature::from_raw(signature_bytes.as_slice())
            .map_err(RemoteSignerError::SignatureError)?;

        Ok(signature)
    }
}
