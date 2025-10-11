//! Module containing a [`TxDeposit`] builder for the Jovian network upgrade transactions.
//!
//! Jovian network upgrade transactions are defined in the [OP Stack Specs][specs].
//!
//! [specs]: https://specs.optimism.io/protocol/Jovian/derivation.html#network-upgrade-automation-transactions

use alloc::{string::String, vec::Vec};
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Address, B256, Bytes, TxKind, U256, address, hex};
use kona_protocol::Predeploys;
use op_alloy_consensus::{TxDeposit, UpgradeDepositSource};

use crate::Hardfork;

/// The Jovian network upgrade transactions.
#[derive(Debug, Default, Clone, Copy)]
pub struct Jovian;

impl Jovian {
    /// The depositor account address.
    pub const DEPOSITOR_ACCOUNT: Address = address!("DeaDDEaDDeAdDeAdDEAdDEaddeAddEAdDEAd0001");

    /// The Enable Jovian Input Method 4Byte Signature.
    ///
    /// Derive this by running `cast sig "setJovian()"`.
    pub const ENABLE_JOVIAN_INPUT: [u8; 4] = hex!("b3d72079");

    /// L1 Block Deployer Address
    pub const L1_BLOCK_DEPLOYER: Address = address!("4210000000000000000000000000000000000006");

    /// The Gas Price Oracle Deployer Address
    pub const GAS_PRICE_ORACLE_DEPLOYER: Address =
        address!("4210000000000000000000000000000000000006");

    /// The new L1 Block Address
    /// This is computed by using go-ethereum's `crypto.CreateAddress` function,
    /// with the L1 Block Deployer Address and nonce 0.
    pub const NEW_L1_BLOCK: Address = address!("3Ba4007f5C922FBb33C454B41ea7a1f11E83df2C");

    /// The Gas Price Oracle Address
    /// This is computed by using go-ethereum's `crypto.CreateAddress` function,
    /// with the Gas Price Oracle Deployer Address and nonce 0.
    pub const GAS_PRICE_ORACLE: Address = address!("3Ba4007f5C922FBb33C454B41ea7a1f11E83df2C");

    /// The Jovian L1 Block Deployer Code Hash
    /// See: <https://specs.optimism.io/protocol/Jovian/derivation.html#l1block-deployment>
    pub const L1_BLOCK_DEPLOYER_CODE_HASH: B256 = alloy_primitives::b256!(
        "0xf7b6ef0de2a53625d467d98c2932a5a5d64ffa1a331ebbde5bb06e2591b5835a"
    );

    /// The Jovian Gas Price Oracle Code Hash
    /// See: <https://specs.optimism.io/protocol/Jovian/derivation.html#gaspriceoracle-deployment>
    pub const GAS_PRICE_ORACLE_CODE_HASH: B256 = alloy_primitives::b256!(
        "0xa7fd95526766fc3ba40ed64aa1b55ad051cb2930df64e11c4a848b81d3a8deaf"
    );

    /// Returns the source hash for the Jovian Gas Price Oracle activation.
    pub fn enable_jovian_source() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: Gas Price Oracle Set Jovian") }
            .source_hash()
    }

    /// Returns the source hash for the deployment of the gas price oracle contract.
    pub fn deploy_gas_price_oracle_source() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: GasPriceOracle Deployment") }
            .source_hash()
    }

    /// Returns the source hash for the deployment of the l1 block contract.
    pub fn deploy_l1_block_source() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: L1Block Deployment") }.source_hash()
    }

    /// Returns the source hash for the update of the l1 block proxy.
    pub fn update_l1_block_source() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: L1Block Proxy Update") }.source_hash()
    }

    /// Returns the source hash for the update of the gas price oracle proxy.
    pub fn update_gas_price_oracle_source() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: GasPriceOracle Proxy Update") }
            .source_hash()
    }

    /// Returns the raw bytecode for the L1 Block deployment.
    pub fn l1_block_deployment_bytecode() -> Bytes {
        hex::decode(include_str!("./bytecode/l1_block_jovian.hex").replace("\n", ""))
            .expect("Expected hex byte string")
            .into()
    }

    /// Returns the gas price oracle deployment bytecode.
    pub fn gas_price_oracle_deployment_bytecode() -> Bytes {
        hex::decode(include_str!("./bytecode/gpo_jovian.hex").replace("\n", ""))
            .expect("Expected hex byte string")
            .into()
    }

    /// Returns the list of [`TxDeposit`]s for the network upgrade.
    pub fn deposits() -> impl Iterator<Item = TxDeposit> {
        ([
            TxDeposit {
                source_hash: Self::deploy_l1_block_source(),
                from: Self::L1_BLOCK_DEPLOYER,
                to: TxKind::Create,
                mint: 0,
                value: U256::ZERO,
                gas_limit: 444_775,
                is_system_transaction: false,
                input: Self::l1_block_deployment_bytecode(),
            },
            TxDeposit {
                source_hash: Self::deploy_gas_price_oracle_source(),
                from: Self::GAS_PRICE_ORACLE_DEPLOYER,
                to: TxKind::Create,
                mint: 0,
                value: U256::ZERO,
                gas_limit: 1_756_783,
                is_system_transaction: false,
                input: Self::gas_price_oracle_deployment_bytecode(),
            },
            TxDeposit {
                source_hash: Self::update_l1_block_source(),
                from: Address::default(),
                to: TxKind::Call(Predeploys::L1_BLOCK_INFO),
                mint: 0,
                value: U256::ZERO,
                gas_limit: 50_000,
                is_system_transaction: false,
                input: super::upgrade_to_calldata(Self::NEW_L1_BLOCK),
            },
            TxDeposit {
                source_hash: Self::update_gas_price_oracle_source(),
                from: Address::default(),
                to: TxKind::Call(Predeploys::GAS_PRICE_ORACLE),
                mint: 0,
                value: U256::ZERO,
                gas_limit: 50_000,
                is_system_transaction: false,
                input: super::upgrade_to_calldata(Self::GAS_PRICE_ORACLE),
            },
            TxDeposit {
                source_hash: Self::enable_jovian_source(),
                from: Self::DEPOSITOR_ACCOUNT,
                to: TxKind::Call(Predeploys::GAS_PRICE_ORACLE),
                mint: 0,
                value: U256::ZERO,
                gas_limit: 90_000,
                is_system_transaction: false,
                input: Self::ENABLE_JOVIAN_INPUT.into(),
            },
        ])
        .into_iter()
    }
}

impl Hardfork for Jovian {
    /// Constructs the network upgrade transactions.
    fn txs(&self) -> impl Iterator<Item = Bytes> + '_ {
        Self::deposits().map(|tx| {
            let mut encoded = Vec::new();
            tx.encode_2718(&mut encoded);
            Bytes::from(encoded)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::check_deployment_code;

    use super::*;
    use alloc::vec;
    use alloy_primitives::b256;

    #[test]
    fn test_l1_block_source_hash() {
        let expected = b256!("98faf23b9795967bc0b1c543144739d50dba3ea40420e77ad6ca9848dbfb62e8");
        assert_eq!(Jovian::deploy_l1_block_source(), expected);
    }

    #[test]
    fn test_gas_price_oracle_source_hash() {
        let expected = b256!("d939cca6eca7bd0ee0c7e89f7e5b5cf7bf6f7afe7b6966bb45dfb95344b31545");
        assert_eq!(Jovian::deploy_gas_price_oracle_source(), expected);
    }

    #[test]
    fn test_l1_block_update_source_hash() {
        let expected = b256!("08447273a4fbce97bc8c515f97ac74efc461f6a4001553712f31ebc11288bad2");
        assert_eq!(Jovian::update_l1_block_source(), expected);
    }

    #[test]
    fn test_gas_price_oracle_update_source_hash() {
        let expected = b256!("46b597e2d8346ed7749b46734074361e0b41a0ab9af7afda5bb4e367e072bcb8");
        assert_eq!(Jovian::update_gas_price_oracle_source(), expected);
    }

    #[test]
    fn test_enable_jovian_source() {
        let expected = b256!("e836db6a959371756f8941be3e962d000f7e12a32e49e2c9ca42ba177a92716c");
        assert_eq!(Jovian::enable_jovian_source(), expected);
    }

    #[test]
    fn test_jovian_txs_encoded() {
        let jovian_upgrade_tx = Jovian.txs().collect::<Vec<_>>();
        assert_eq!(jovian_upgrade_tx.len(), 5);

        let expected_txs: Vec<Bytes> = vec![
            hex::decode(include_str!("./bytecode/jovian_tx_0.hex").replace("\n", ""))
                .unwrap()
                .into(),
            hex::decode(include_str!("./bytecode/jovian_tx_1.hex").replace("\n", ""))
                .unwrap()
                .into(),
            hex::decode(include_str!("./bytecode/jovian_tx_2.hex").replace("\n", ""))
                .unwrap()
                .into(),
            hex::decode(include_str!("./bytecode/jovian_tx_3.hex").replace("\n", ""))
                .unwrap()
                .into(),
            hex::decode(include_str!("./bytecode/jovian_tx_4.hex").replace("\n", ""))
                .unwrap()
                .into(),
        ];
        for (i, expected) in expected_txs.iter().enumerate() {
            assert_eq!(jovian_upgrade_tx[i], *expected);
        }
    }

    #[test]
    fn test_verify_jovian_l1_block_deployment_code_hash() {
        let txs = Jovian::deposits().collect::<Vec<_>>();
        check_deployment_code(
            txs[0].clone(),
            Jovian::NEW_L1_BLOCK,
            Jovian::L1_BLOCK_DEPLOYER_CODE_HASH,
        );
    }
    #[test]
    fn test_verify_jovian_gas_price_oracle_deployment_code_hash() {
        let txs = Jovian::deposits().collect::<Vec<_>>();

        check_deployment_code(
            txs[1].clone(),
            Jovian::GAS_PRICE_ORACLE,
            Jovian::GAS_PRICE_ORACLE_CODE_HASH,
        );
    }
}
