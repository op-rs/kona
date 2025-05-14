use alloy_primitives::B256;
use alloy_rlp:: {RlpDecodable, RlpEncodable};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use reth_db_api::DatabaseError;
use reth_db_api::{Tables};
use reth_db_api::table::{Encode, Decode};
use kona_interop::ExecutingMessage;


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

#[derive(Clone, Debug, PartialEq, Eq, Default, RlpEncodable, RlpDecodable)]
#[rlp(trailing)]
pub struct LogValue {
    pub hash: B256,
    pub executing_message: Option<ExecutingMessageValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, RlpEncodable, RlpDecodable)]
pub struct ExecutingMessageValue {
    hash: B256,
    timestamp: u64,
}

// #[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
// pub struct BlockRef {
//     pub hash: B256,
//     pub number: u64,
//     pub parent_hash: B256,
//     pub time: u64,
// }
//
//
// use reth_db::table::{Table, TableInfo};
// use serde::{Deserialize, Serialize};
//
// pub struct LogsTable;
// impl Table for LogsTable {
//     type Key = LogKey;
//     type Value = LogValue;
//     fn table_info() -> TableInfo {
//         TableInfo { name: "Logs" }
//     }
// }
//
// pub struct BlockRefTable;
// impl Table for BlockRefTable {
//     type Key = (u64, u64); // (chain_id, block_number)
//     type Value = BlockRef;
//     fn table_info() -> TableInfo {
//         TableInfo { name: "BlockRef" }
//     }
// }
