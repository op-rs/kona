use alloy_primitives::B256;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use alloy_rlp::{bytes};
use reth_db_api::{DatabaseError};
use reth_db_api::table::{Encode, Decode, Compress, Decompress, TableInfo};
use reth_db::table::{Table};
use bytes::BufMut;
use reth_codecs::Compact;

#[derive(Ord, PartialOrd, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct LogKey {
    pub chain_id: u64,
    pub block_number: u64,
    pub log_index: u32,
}

impl Encode for LogKey {
    type Encoded = [u8; 20];

    fn encode(self) -> Self::Encoded {
        let mut buf = [0u8; 20];
        buf[..8].copy_from_slice(&self.chain_id.to_be_bytes());
        buf[8..16].copy_from_slice(&self.block_number.to_be_bytes());
        buf[16..20].copy_from_slice(&self.log_index.to_be_bytes());
        buf
    }
}

impl Decode for LogKey {
    fn decode(value: &[u8]) -> Result<Self, reth_db_api::DatabaseError> {
        if value.len() != 20 {
            return Err(DatabaseError::Decode);
        }

        let chain_id = u64::from_be_bytes(value[0..8].try_into().unwrap());
        let block_number = u64::from_be_bytes(value[8..16].try_into().unwrap());
        let log_index = u32::from_be_bytes(value[16..20].try_into().unwrap());

        Ok(LogKey {
            chain_id,
            block_number,
            log_index,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LogValue {
    pub hash: B256,
    pub executing_message_hash: B256,
    pub timestamp: u64,
}

// TODO: check if we can use derive
impl Compress for LogValue {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: BufMut + AsMut<[u8]>>(&self, buf: &mut B) {
        let _ = Compact::to_compact(self, buf);
    }
}

// TODO: check if we can use derive
impl Decompress for LogValue {
    fn decompress(value: &[u8]) -> Result<Self, DatabaseError> {
        let (obj, _) = Compact::from_compact(value, value.len());
        Ok(obj)
    }
}

// TODO: use derive
impl Compact for LogValue {
    fn to_compact<B: BufMut + AsMut<[u8]>>(&self, out: &mut B) -> usize {
        let encoded = bincode::serialize(self).expect("bincode never fails");
        out.put_slice(&encoded);
        encoded.len()
    }

    fn from_compact(bytes: &[u8], _len: usize) -> (Self, &[u8]) {
        let value: Self = bincode::deserialize(bytes).expect("bincode deserialize failed");
        (value, &[])
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct LogEntries;
impl Table for LogEntries {
    const NAME: &'static str = "log_entries";
    const DUPSORT: bool = false;

    type Key = LogKey;
    type Value = LogValue;
}

impl TableInfo for LogEntries {
    fn name(&self) -> &'static str {
        LogEntries::NAME
    }
    fn is_dupsort(&self) -> bool {
        LogEntries::DUPSORT
    }
}