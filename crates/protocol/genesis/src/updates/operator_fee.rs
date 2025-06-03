//! The Operator Fee update type.

use alloy_primitives::LogData;
use alloy_sol_types::{SolType, sol};

use crate::{OperatorFeeUpdateError, SystemConfig, SystemConfigLog};

/// The Operator Fee update type.
#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OperatorFeeUpdate {
    /// The operator fee scalar.
    pub operator_fee_scalar: u32,
    /// The operator fee constant.
    pub operator_fee_constant: u64,
}

impl OperatorFeeUpdate {
    /// Applies the update to the [`SystemConfig`].
    pub const fn apply(&self, config: &mut SystemConfig) {
        config.operator_fee_scalar = Some(self.operator_fee_scalar);
        config.operator_fee_constant = Some(self.operator_fee_constant);
    }
}

impl TryFrom<&SystemConfigLog> for OperatorFeeUpdate {
    type Error = OperatorFeeUpdateError;

    fn try_from(log: &SystemConfigLog) -> Result<Self, Self::Error> {
        let LogData { data, .. } = &log.log.data;
        if data.len() != 96 {
            return Err(OperatorFeeUpdateError::InvalidDataLen(data.len()));
        }

        let Ok(pointer) = <sol!(uint64)>::abi_decode_validate(&data[0..32]) else {
            return Err(OperatorFeeUpdateError::PointerDecodingError);
        };
        if pointer != 32 {
            return Err(OperatorFeeUpdateError::InvalidDataPointer(pointer));
        }

        let Ok(length) = <sol!(uint64)>::abi_decode_validate(&data[32..64]) else {
            return Err(OperatorFeeUpdateError::LengthDecodingError);
        };
        if length != 32 {
            return Err(OperatorFeeUpdateError::InvalidDataLength(length));
        }

        // The operator fee scalar and constant are
        // packed into a single u256 as follows:
        //
        // | Bytes    | Actual Size | Variable |
        // |----------|-------------|----------|
        // | 0 .. 24  | uint32      | scalar   |
        // | 24 .. 32 | uint64      | constant |
        // |----------|-------------|----------|

        let mut be_bytes = [0u8; 4];
        be_bytes[0..4].copy_from_slice(&data[84..88]);
        let operator_fee_scalar = u32::from_be_bytes(be_bytes);

        let mut be_bytes = [0u8; 8];
        be_bytes[0..8].copy_from_slice(&data[88..96]);
        let operator_fee_constant = u64::from_be_bytes(be_bytes);

        Ok(Self { operator_fee_scalar, operator_fee_constant })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CONFIG_UPDATE_EVENT_VERSION_0, CONFIG_UPDATE_TOPIC};
    use alloc::vec;
    use alloy_primitives::{Address, B256, Bytes, Log, LogData, hex};

    #[test]
    fn test_operator_fee_update_try_from() {
        let log = Log {
            address: Address::ZERO,
            data: LogData::new_unchecked(
                vec![], // Topics aren't checked
                hex!("0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000babe000000000000beef").into()
            )
        };

        let system_log = SystemConfigLog::new(log, false);
        let update = OperatorFeeUpdate::try_from(&system_log).unwrap();

        assert_eq!(update.operator_fee_scalar, 0xbabe_u32);
        assert_eq!(update.operator_fee_constant, 0xbeef_u64);
    }

    #[test]
    fn test_operator_fee_update_invalid_data_len() {
        let log =
            Log { address: Address::ZERO, data: LogData::new_unchecked(vec![], Bytes::default()) };
        let system_log = SystemConfigLog::new(log, false);
        let err = OperatorFeeUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, OperatorFeeUpdateError::InvalidDataLen(0));
    }

    #[test]
    fn test_operator_fee_update_pointer_decoding_error() {
        let log = Log {
            address: Address::ZERO,
            data: LogData::new_unchecked(
                vec![
                    CONFIG_UPDATE_TOPIC,
                    CONFIG_UPDATE_EVENT_VERSION_0,
                    B256::ZERO,
                ],
                hex!("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000babe0000beef").into()
            )
        };

        let system_log = SystemConfigLog::new(log, false);
        let err = OperatorFeeUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, OperatorFeeUpdateError::PointerDecodingError);
    }

    #[test]
    fn test_operator_fee_update_invalid_pointer_length() {
        let log = Log {
            address: Address::ZERO,
            data: LogData::new_unchecked(
                vec![
                    CONFIG_UPDATE_TOPIC,
                    CONFIG_UPDATE_EVENT_VERSION_0,
                    B256::ZERO,
                ],
                hex!("000000000000000000000000000000000000000000000000000000000000002100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000babe0000beef").into()
            )
        };

        let system_log = SystemConfigLog::new(log, false);
        let err = OperatorFeeUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, OperatorFeeUpdateError::InvalidDataPointer(33));
    }

    #[test]
    fn test_operator_fee_update_length_decoding_error() {
        let log = Log {
            address: Address::ZERO,
            data: LogData::new_unchecked(
                vec![
                    CONFIG_UPDATE_TOPIC,
                    CONFIG_UPDATE_EVENT_VERSION_0,
                    B256::ZERO,
                ],
                hex!("0000000000000000000000000000000000000000000000000000000000000020FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0000000000000000000000000000000000000000000000000000babe0000beef").into()
            )
        };

        let system_log = SystemConfigLog::new(log, false);
        let err = OperatorFeeUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, OperatorFeeUpdateError::LengthDecodingError);
    }

    #[test]
    fn test_operator_fee_update_invalid_data_length() {
        let log = Log {
            address: Address::ZERO,
            data: LogData::new_unchecked(
                vec![
                    CONFIG_UPDATE_TOPIC,
                    CONFIG_UPDATE_EVENT_VERSION_0,
                    B256::ZERO,
                ],
                hex!("000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000210000000000000000000000000000000000000000000000000000babe0000beef").into()
            )
        };

        let system_log = SystemConfigLog::new(log, false);
        let err = OperatorFeeUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, OperatorFeeUpdateError::InvalidDataLength(33));
    }
}
