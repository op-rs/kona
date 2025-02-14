//! The batcher update type.

use alloy_primitives::Address;
use alloy_sol_types::{sol, SolType};

use crate::{BatcherUpdateError, SystemConfig, SystemConfigLog};

/// The batcher update type.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BatcherUpdate {
    /// The batcher address.
    pub batcher_address: Address,
}

impl BatcherUpdate {
    /// Applies the update to the [`SystemConfig`].
    pub fn apply(&self, config: &mut SystemConfig) {
        config.batcher_address = self.batcher_address;
    }
}

impl TryFrom<&SystemConfigLog> for BatcherUpdate {
    type Error = BatcherUpdateError;

    fn try_from(log: &SystemConfigLog) -> Result<Self, Self::Error> {
        let log = &log.log;
        if log.data.data.len() != 96 {
            return Err(BatcherUpdateError::InvalidDataLen(log.data.data.len()));
        }

        let Ok(pointer) = <sol!(uint64)>::abi_decode(&log.data.data[0..32], true) else {
            return Err(BatcherUpdateError::PointerDecodingError);
        };
        if pointer != 32 {
            return Err(BatcherUpdateError::InvalidDataPointer(pointer));
        }
        let Ok(length) = <sol!(uint64)>::abi_decode(&log.data.data[32..64], true) else {
            return Err(BatcherUpdateError::LengthDecodingError);
        };
        if length != 32 {
            return Err(BatcherUpdateError::InvalidDataLength(length));
        }

        let Ok(batcher_address) = <sol!(address)>::abi_decode(&log.data.data[64..], true) else {
            return Err(BatcherUpdateError::BatcherAddressDecodingError);
        };

        Ok(Self { batcher_address })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CONFIG_UPDATE_EVENT_VERSION_0, CONFIG_UPDATE_TOPIC};
    use alloc::vec;
    use alloy_primitives::{address, hex, Bytes, Log, LogData, B256};

    #[test]
    fn test_batcher_update_try_from() {
        let update_type = B256::ZERO;

        let log = Log {
            address: Address::ZERO,
            data: LogData::new_unchecked(
                vec![
                    CONFIG_UPDATE_TOPIC,
                    CONFIG_UPDATE_EVENT_VERSION_0,
                    update_type,
                ],
                hex!("00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000beef").into()
            )
        };

        let system_log = SystemConfigLog::new(log, false);
        let update = BatcherUpdate::try_from(&system_log).unwrap();
        assert_eq!(update.batcher_address, address!("000000000000000000000000000000000000bEEF"),);
    }

    #[test]
    fn test_batcher_update_invalid_data_len() {
        let log =
            Log { address: Address::ZERO, data: LogData::new_unchecked(vec![], Bytes::default()) };
        let system_log = SystemConfigLog::new(log, false);
        let err = BatcherUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, BatcherUpdateError::InvalidDataLen(0));
    }

    #[test]
    fn test_batcher_update_pointer_decoding_error() {
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
        let err = BatcherUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, BatcherUpdateError::PointerDecodingError);
    }

    #[test]
    fn test_batcher_update_invalid_pointer_length() {
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
        let err = BatcherUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, BatcherUpdateError::InvalidDataPointer(33));
    }

    #[test]
    fn test_batcher_update_length_decoding_error() {
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
        let err = BatcherUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, BatcherUpdateError::LengthDecodingError);
    }

    #[test]
    fn test_batcher_update_invalid_data_length() {
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
        let err = BatcherUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, BatcherUpdateError::InvalidDataLength(33));
    }

    #[test]
    fn test_batcher_update_batcher_decoding_error() {
        let log = Log {
            address: Address::ZERO,
            data: LogData::new_unchecked(
                vec![
                    CONFIG_UPDATE_TOPIC,
                    CONFIG_UPDATE_EVENT_VERSION_0,
                    B256::ZERO,
                ],
                hex!("00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000020FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").into()
            )
        };

        let system_log = SystemConfigLog::new(log, false);
        let err = BatcherUpdate::try_from(&system_log).unwrap_err();
        assert_eq!(err, BatcherUpdateError::BatcherAddressDecodingError);
    }
}
