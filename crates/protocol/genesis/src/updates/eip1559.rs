//! The EIP-1559 update type.

use alloy_sol_types::{sol, SolType};

use crate::{EIP1559UpdateError, SystemConfig, SystemConfigLog};

/// The EIP-1559 update type.
#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Eip1559Update {
    /// The EIP-1559 denominator.
    pub eip1559_denominator: u32,
    /// The EIP-1559 elasticity multiplier.
    pub eip1559_elasticity: u32,
}

impl Eip1559Update {
    /// Applies the update to the [`SystemConfig`].
    pub fn apply(&self, config: &mut SystemConfig) {
        config.eip1559_denominator = Some(self.eip1559_denominator);
        config.eip1559_elasticity = Some(self.eip1559_elasticity);
    }
}

impl TryFrom<&SystemConfigLog> for Eip1559Update {
    type Error = EIP1559UpdateError;

    fn try_from(log: &SystemConfigLog) -> Result<Self, Self::Error> {
        let log = &log.log;
        if log.data.data.len() != 96 {
            return Err(EIP1559UpdateError::InvalidDataLen(log.data.data.len()));
        }

        let Ok(pointer) = <sol!(uint64)>::abi_decode(&log.data.data[0..32], true) else {
            return Err(EIP1559UpdateError::PointerDecodingError);
        };
        if pointer != 32 {
            return Err(EIP1559UpdateError::InvalidDataPointer(pointer));
        }
        let Ok(length) = <sol!(uint64)>::abi_decode(&log.data.data[32..64], true) else {
            return Err(EIP1559UpdateError::LengthDecodingError);
        };
        if length != 32 {
            return Err(EIP1559UpdateError::InvalidDataLength(length));
        }

        let Ok(eip1559_params) = <sol!(uint64)>::abi_decode(&log.data.data[64..], true) else {
            return Err(EIP1559UpdateError::EIP1559DecodingError);
        };

        Ok(Self {
            eip1559_denominator: (eip1559_params >> 32) as u32,
            eip1559_elasticity: eip1559_params as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CONFIG_UPDATE_EVENT_VERSION_0, CONFIG_UPDATE_TOPIC};
    use alloc::vec;
    use alloy_primitives::{hex, Address, Bytes, Log, LogData, B256};

    #[test]
    fn test_eip1559_update_try_from() {
        let update_type = B256::ZERO;

        let log = Log {
            address: Address::ZERO,
            data: LogData::new_unchecked(
                vec![
                    CONFIG_UPDATE_TOPIC,
                    CONFIG_UPDATE_EVENT_VERSION_0,
                    update_type,
                ],
                hex!("000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000babe0000beef").into()
            )
        };

        let system_log = SystemConfigLog::new(log, false);
        let update = Eip1559Update::try_from(&system_log).unwrap();

        assert_eq!(update.eip1559_denominator, 0xbabe_u32);
        assert_eq!(update.eip1559_elasticity, 0xbeef_u32);
    }

    #[test]
    fn test_eip1559_update_invalid_data_len() {
        let log =
            Log { address: Address::ZERO, data: LogData::new_unchecked(vec![], Bytes::default()) };
        let system_log = SystemConfigLog::new(log, false);
        let err = Eip1559Update::try_from(&system_log).unwrap_err();
        assert_eq!(err, EIP1559UpdateError::InvalidDataLen(0));
    }

    #[test]
    fn test_eip1559_update_pointer_decoding_error() {
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
        let err = Eip1559Update::try_from(&system_log).unwrap_err();
        assert_eq!(err, EIP1559UpdateError::PointerDecodingError);
    }

    #[test]
    fn test_eip1559_update_invalid_pointer_length() {
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
        let err = Eip1559Update::try_from(&system_log).unwrap_err();
        assert_eq!(err, EIP1559UpdateError::InvalidDataPointer(33));
    }

    #[test]
    fn test_eip1559_update_length_decoding_error() {
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
        let err = Eip1559Update::try_from(&system_log).unwrap_err();
        assert_eq!(err, EIP1559UpdateError::LengthDecodingError);
    }

    #[test]
    fn test_eip1559_update_invalid_data_length() {
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
        let err = Eip1559Update::try_from(&system_log).unwrap_err();
        assert_eq!(err, EIP1559UpdateError::InvalidDataLength(33));
    }

    #[test]
    fn test_eip1559_update_eip1559_decoding_error() {
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
        let err = Eip1559Update::try_from(&system_log).unwrap_err();
        assert_eq!(err, EIP1559UpdateError::EIP1559DecodingError);
    }
}
