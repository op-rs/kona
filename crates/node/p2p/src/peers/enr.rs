//! Contains the Optimism consensus-layer ENR Type.

use alloy_rlp::{Decodable, Encodable};
use discv5::Enr;
use unsigned_varint::{decode, encode};

/// Validates the [`Enr`] for the OP Stack.
#[derive(Debug, derive_more::Display, Clone, Default, PartialEq, Eq)]
pub enum EnrValidation {
    /// Missing OP Stack ENR key ("opstack").
    #[display("Missing OP Stack ENR key: {_0}")]
    MissingKey(String),
    /// Failed to decode the OP Stack ENR Value into an [`OpStackEnr`].
    #[display("Failed to decode the OP Stack ENR Value: {_0}")]
    DecodeError(String),
    /// Invalid Chain ID.
    #[display("Invalid Chain ID: {_0}")]
    InvalidChainId(u64),
    /// Invalid Version.
    #[display("Invalid Version: {_0}")]
    InvalidVersion(u64),
    /// Valid ENR.
    #[default]
    #[display("Valid ENR")]
    Valid,
}

impl EnrValidation {
    /// Validates the [`Enr`] for the OP Stack.
    pub fn validate(enr: &Enr, chain_id: u64) -> Self {
        let Some(mut opstack) = enr.get_raw_rlp(OpStackEnr::OP_CL_KEY) else {
            return Self::MissingKey(OpStackEnr::OP_CL_KEY.to_string());
        };

        let opstack_enr = match OpStackEnr::decode(&mut opstack) {
            Ok(val) => val,
            Err(e) => {
                return Self::DecodeError(format!("Failed to decode: {e}"));
            }
        };

        if opstack_enr.chain_id != chain_id {
            return Self::InvalidChainId(opstack_enr.chain_id);
        }

        if opstack_enr.version != 0 {
            return Self::InvalidVersion(opstack_enr.version);
        }

        Self::Valid
    }

    /// Returns `true` if the ENR is valid.
    pub const fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Returns `true` if the ENR is invalid.
    pub const fn is_invalid(&self) -> bool {
        !self.is_valid()
    }
}

/// The unique L2 network identifier
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct OpStackEnr {
    /// Chain ID
    pub chain_id: u64,
    /// The version. Always set to 0.
    pub version: u64,
}

impl OpStackEnr {
    /// The [`Enr`] key literal string for the consensus layer.
    pub const OP_CL_KEY: &str = "opstack";

    /// Constructs an [`OpStackEnr`] from a chain id.
    pub const fn from_chain_id(chain_id: u64) -> Self {
        Self { chain_id, version: 0 }
    }

    /// Returns `true` if a node [`Enr`] contains an `opstack` key and is on the same network.
    pub fn is_valid_node(node: &Enr, chain_id: u64) -> bool {
        node.get_raw_rlp(Self::OP_CL_KEY).is_some_and(|mut opstack| {
            Self::decode(&mut opstack)
                .is_ok_and(|opstack| opstack.chain_id == chain_id && opstack.version == 0)
        })
    }
}

impl Encodable for OpStackEnr {
    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        let mut chain_id_buf = encode::u128_buffer();
        let chain_id_slice = encode::u128(self.chain_id as u128, &mut chain_id_buf);

        let mut version_buf = encode::u128_buffer();
        let version_slice = encode::u128(self.version as u128, &mut version_buf);

        let opstack = [chain_id_slice, version_slice].concat();
        alloy_primitives::Bytes::from(opstack).encode(out);
    }
}

impl Decodable for OpStackEnr {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let bytes = alloy_primitives::Bytes::decode(buf)?;
        let (chain_id, rest) = decode::u64(&bytes)
            .map_err(|_| alloy_rlp::Error::Custom("could not decode chain id"))?;
        let (version, _) =
            decode::u64(rest).map_err(|_| alloy_rlp::Error::Custom("could not decode chain id"))?;
        Ok(Self { chain_id, version })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Bytes, bytes};
    use discv5::enr::CombinedKey;

    #[test]
    #[cfg(feature = "arbitrary")]
    fn roundtrip_op_stack_enr() {
        arbtest::arbtest(|u| {
            let op_stack_enr = OpStackEnr::from_chain_id(u.arbitrary()?);
            let bytes = alloy_rlp::encode(op_stack_enr).to_vec();
            let decoded = OpStackEnr::decode(&mut &bytes[..]).unwrap();
            assert_eq!(decoded, op_stack_enr);
            Ok(())
        });
    }

    #[test]
    fn test_is_valid_node() {
        let key = CombinedKey::generate_secp256k1();
        let mut enr = Enr::builder().build(&key).unwrap();
        let op_stack_enr = OpStackEnr::from_chain_id(10);
        let mut op_stack_bytes = Vec::new();
        op_stack_enr.encode(&mut op_stack_bytes);
        enr.insert_raw_rlp(OpStackEnr::OP_CL_KEY, op_stack_bytes.into(), &key).unwrap();
        assert!(OpStackEnr::is_valid_node(&enr, 10));
        assert!(!OpStackEnr::is_valid_node(&enr, 11));
    }

    #[test]
    fn test_is_valid_node_invalid_version() {
        let key = CombinedKey::generate_secp256k1();
        let mut enr = Enr::builder().build(&key).unwrap();
        let mut op_stack_enr = OpStackEnr::from_chain_id(10);
        op_stack_enr.version = 1;
        let mut op_stack_bytes = Vec::new();
        op_stack_enr.encode(&mut op_stack_bytes);
        enr.insert_raw_rlp(OpStackEnr::OP_CL_KEY, op_stack_bytes.into(), &key).unwrap();
        assert!(!OpStackEnr::is_valid_node(&enr, 10));
    }

    #[test]
    fn test_op_mainnet_enr() {
        let op_enr = OpStackEnr::from_chain_id(10);
        let bytes = alloy_rlp::encode(op_enr).to_vec();
        assert_eq!(Bytes::from(bytes.clone()), bytes!("820A00"));
        let decoded = OpStackEnr::decode(&mut &bytes[..]).unwrap();
        assert_eq!(decoded, op_enr);
    }

    #[test]
    fn test_base_mainnet_enr() {
        let base_enr = OpStackEnr::from_chain_id(8453);
        let bytes = alloy_rlp::encode(base_enr).to_vec();
        assert_eq!(Bytes::from(bytes.clone()), bytes!("83854200"));
        let decoded = OpStackEnr::decode(&mut &bytes[..]).unwrap();
        assert_eq!(decoded, base_enr);
    }
}
