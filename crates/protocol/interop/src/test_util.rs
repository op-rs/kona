//! Test utilities for `kona-interop`.

#![allow(missing_docs, unreachable_pub)]

use crate::{traits::InteropProvider, ExecutingMessage, MessageIdentifier, CROSS_L2_INBOX_ADDRESS};
use alloy_consensus::{Header, Receipt, ReceiptWithBloom, Sealed};
use alloy_primitives::{map::HashMap, Address, Bytes, Log, LogData, B256, U256};
use alloy_sol_types::{SolEvent, SolValue};
use async_trait::async_trait;
use op_alloy_consensus::OpReceiptEnvelope;

#[derive(Debug, Clone, Default)]
pub(crate) struct MockInteropProvider {
    pub headers: HashMap<u64, HashMap<u64, Sealed<Header>>>,
    pub receipts: HashMap<u64, HashMap<u64, Vec<OpReceiptEnvelope>>>,
}

impl MockInteropProvider {
    pub const fn new(
        headers: HashMap<u64, HashMap<u64, Sealed<Header>>>,
        receipts: HashMap<u64, HashMap<u64, Vec<OpReceiptEnvelope>>>,
    ) -> Self {
        Self { headers, receipts }
    }
}

#[derive(thiserror::Error, Debug, Eq, PartialEq)]
#[error("Mock interop provider error")]
pub struct InteropProviderError;

#[async_trait]
impl InteropProvider for MockInteropProvider {
    type Error = InteropProviderError;

    /// Fetch a [Header] by its number.
    async fn header_by_number(&self, chain_id: u64, number: u64) -> Result<Header, Self::Error> {
        Ok(self
            .headers
            .get(&chain_id)
            .and_then(|headers| headers.get(&number))
            .unwrap()
            .inner()
            .clone())
    }

    /// Fetch all receipts for a given block by number.
    async fn receipts_by_number(
        &self,
        chain_id: u64,
        number: u64,
    ) -> Result<Vec<OpReceiptEnvelope>, Self::Error> {
        Ok(self.receipts.get(&chain_id).and_then(|receipts| receipts.get(&number)).unwrap().clone())
    }

    /// Fetch all receipts for a given block by hash.
    async fn receipts_by_hash(
        &self,
        chain_id: u64,
        block_hash: B256,
    ) -> Result<Vec<OpReceiptEnvelope>, Self::Error> {
        Ok(self
            .receipts
            .get(&chain_id)
            .and_then(|receipts| {
                let headers = self.headers.get(&chain_id).unwrap();
                let number =
                    headers.values().find(|header| header.hash() == block_hash).unwrap().number;
                receipts.get(&number)
            })
            .unwrap()
            .clone())
    }
}

pub struct SuperchainBuilder {
    chains: HashMap<u64, ChainBuilder>,
    timestamp: u64,
}

pub struct ChainBuilder {
    header: Header,
    receipts: Vec<OpReceiptEnvelope>,
}

impl SuperchainBuilder {
    pub fn new(timestamp: u64) -> Self {
        Self { chains: HashMap::default(), timestamp }
    }

    pub fn chain(&mut self, chain_id: u64) -> &mut ChainBuilder {
        self.chains.entry(chain_id).or_insert_with(|| ChainBuilder::new(self.timestamp))
    }

    /// Builds the scenario into the format needed for testing
    pub fn build(self) -> (Vec<(u64, Sealed<Header>)>, MockInteropProvider) {
        let mut headers_map = HashMap::default();
        let mut receipts_map = HashMap::default();
        let mut sealed_headers = Vec::new();

        for (chain_id, chain) in self.chains {
            let header = chain.header;
            let header_hash = header.hash_slow();
            let sealed_header = header.seal(header_hash);

            let mut chain_headers = HashMap::default();
            chain_headers.insert(0, sealed_header.clone());
            headers_map.insert(chain_id, chain_headers);

            let mut chain_receipts = HashMap::default();
            chain_receipts.insert(0, chain.receipts);
            receipts_map.insert(chain_id, chain_receipts);

            sealed_headers.push((chain_id, sealed_header));
        }

        (sealed_headers, MockInteropProvider::new(headers_map, receipts_map))
    }
}

impl ChainBuilder {
    pub fn new(timestamp: u64) -> Self {
        Self { header: Header { timestamp, ..Default::default() }, receipts: Vec::new() }
    }

    pub fn add_initiating_message(&mut self, message_data: Bytes) -> &mut Self {
        let receipt = OpReceiptEnvelope::Eip1559(ReceiptWithBloom {
            receipt: Receipt {
                logs: vec![Log {
                    address: Address::ZERO,
                    data: LogData::new(vec![], message_data).unwrap(),
                }],
                ..Default::default()
            },
            ..Default::default()
        });
        self.receipts.push(receipt);
        self
    }

    pub fn add_executing_message(
        &mut self,
        message_hash: B256,
        origin_log_index: u64,
        origin_chain_id: u64,
        origin_timestamp: u64,
    ) -> &mut Self {
        self.add_executing_message_with_origin(
            message_hash,
            Address::ZERO,
            origin_log_index,
            origin_chain_id,
            origin_timestamp,
        )
    }

    pub fn add_executing_message_with_origin(
        &mut self,
        message_hash: B256,
        origin_address: Address,
        origin_log_index: u64,
        origin_chain_id: u64,
        origin_timestamp: u64,
    ) -> &mut Self {
        let receipt = OpReceiptEnvelope::Eip1559(ReceiptWithBloom {
            receipt: Receipt {
                logs: vec![Log {
                    address: CROSS_L2_INBOX_ADDRESS,
                    data: LogData::new(
                        vec![ExecutingMessage::SIGNATURE_HASH, message_hash],
                        MessageIdentifier {
                            origin: origin_address,
                            blockNumber: U256::ZERO,
                            logIndex: U256::from(origin_log_index),
                            timestamp: U256::from(origin_timestamp),
                            chainId: U256::from(origin_chain_id),
                        }
                        .abi_encode()
                        .into(),
                    )
                    .unwrap(),
                }],
                ..Default::default()
            },
            ..Default::default()
        });
        self.receipts.push(receipt);
        self
    }
}
