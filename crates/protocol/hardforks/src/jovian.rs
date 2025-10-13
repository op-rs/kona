//! Module containing a [`TxDeposit`] builder for the Jovian network upgrade transactions.
//!
//! Jovian network upgrade transactions are defined in the [OP Stack Specs][specs].
//!
//! [specs]: https://specs.optimism.io/protocol/jovian/derivation.html#network-upgrade-automation-transactions

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

    /// Zero address
    pub const ZERO_ADDRESS: Address = address!("0x0000000000000000000000000000000000000000");

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
    /// See: <https://specs.optimism.io/protocol/jovian/derivation.html#l1block-deployment>
    pub const L1_BLOCK_DEPLOYER_CODE_HASH: B256 = alloy_primitives::b256!(
        "0x98faf23b9795967bc0b1c543144739d50dba3ea40420e77ad6ca9848dbfb62e8"
    );

    /// The Jovian Gas Price Oracle Code Hash
    /// See: <https://specs.optimism.io/protocol/jovian/derivation.html#gaspriceoracle-deployment>
    pub const GAS_PRICE_ORACLE_CODE_HASH: B256 = alloy_primitives::b256!(
        "0xd939cca6eca7bd0ee0c7e89f7e5b5cf7bf6f7afe7b6966bb45dfb95344b31545"
    );

    /// Returns the source hash for the Jovian Gas Price Oracle activation.
    pub fn enable_jovian_source() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: Gas Price Oracle Set Jovian") }
            .source_hash()
    }

    /// Returns the source hash for the EIP-2935 block hash history contract deployment.
    pub fn block_hash_history_contract_source() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: EIP-2935 Contract Deployment") }
            .source_hash()
    }

    /// Returns the source hash for the deployment of the l1 block contract.
    pub fn deploy_l1_block_source() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: L1Block Deployment") }.source_hash()
    }

    /// Returns the source hash for the deployment of the gas price oracle contract.
    pub fn l1_block_proxy_update() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: L1Block Proxy Update") }.source_hash()
    }

    /// Returns the source hash for the deployment of the operator fee vault contract.
    pub fn gas_price_oracle() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: GasPriceOracle Deployment") }
            .source_hash()
    }

    /// Returns the source hash for the update of the l1 block proxy.
    pub fn gas_price_oracle_proxy_update() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: GasPriceOracle Proxy Update") }
            .source_hash()
    }

    /// Returns the source hash to the enable the gas price oracle for Jovian.
    pub fn gas_price_oracle_enable_jovian() -> B256 {
        UpgradeDepositSource { intent: String::from("Jovian: Gas Price Oracle Set Jovian") }
            .source_hash()
    }

    /// Returns the raw bytecode for the L1 Block deployment.
    pub fn l1_block_deployment_bytecode() -> Bytes {
        hex::decode(include_str!("./bytecode/jovian-l1-block-deployment.hex").replace("\n", ""))
            .expect("Expected hex byte string")
            .into()
    }

    /// Returns the gas price oracle deployment bytecode.
    pub fn l1_proxy_update_bytecode() -> Bytes {
        hex::decode("0x3659cfe60000000000000000000000003ba4007f5c922fbb33c454b41ea7a1f11e83df2c")
            .expect("Expected hex byte string")
            .into()
    }

    /// Returns the gas price oracle deployment bytecode.
    pub fn gas_price_oracle_deployment_bytecode() -> Bytes {
        hex::decode(
            include_str!("./bytecode/jovian-gas-price-oracle-deployment.hex").replace("\n", ""),
        )
        .expect("Expected hex byte string")
        .into()
    }

    /// Returns the bytecode for the gas price oracle proxy update.
    pub fn gas_price_oracle_proxy_update_bytecode() -> Bytes {
        hex::decode("0x3659cfe60000000000000000000000003ba4007f5c922fbb33c454b41ea7a1f11e83df2c")
            .expect("Expected hex byte string")
            .into()
    }

    /// Returns the bytecode to enable the gas price oracle for Jovian.
    pub fn gas_price_oracle_enable_jovian_bytecode() -> Bytes {
        hex::decode("0xb3d72079").expect("Expected hex byte string").into()
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
                source_hash: Self::l1_block_proxy_update(),
                from: Self::ZERO_ADDRESS,
                to: TxKind::Create,
                mint: 0,
                value: U256::ZERO,
                gas_limit: 50_000,
                is_system_transaction: false,
                input: Self::l1_proxy_update_bytecode(),
            },
            TxDeposit {
                source_hash: Self::gas_price_oracle(),
                from: Self::GAS_PRICE_ORACLE_DEPLOYER,
                to: TxKind::Create,
                mint: 0,
                value: U256::ZERO,
                gas_limit: 1_756_783,
                is_system_transaction: false,
                input: Self::gas_price_oracle_deployment_bytecode(),
            },
            TxDeposit {
                source_hash: Self::gas_price_oracle_proxy_update(),
                from: Self::ZERO_ADDRESS,
                to: TxKind::Call(Predeploys::GAS_PRICE_ORACLE),
                mint: 0,
                value: U256::ZERO,
                gas_limit: 50_000,
                is_system_transaction: false,
                input: Self::gas_price_oracle_proxy_update_bytecode(),
            },
            TxDeposit {
                source_hash: Self::gas_price_oracle_enable_jovian(),
                from: Self::DEPOSITOR_ACCOUNT,
                to: TxKind::Call(Predeploys::GAS_PRICE_ORACLE),
                mint: 0,
                value: U256::ZERO,
                gas_limit: 90_000,
                is_system_transaction: false,
                input: Self::gas_price_oracle_enable_jovian_bytecode(),
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
    use alloy_primitives::b256;

    #[test]
    fn test_l1_block_source_hash() {
        let expected = b256!("98faf23b9795967bc0b1c543144739d50dba3ea40420e77ad6ca9848dbfb62e8");
        assert_eq!(Jovian::deploy_l1_block_source(), expected);
    }

    #[test]
    fn test_l1_block_proxy_update_source_hash() {
        let expected = b256!("08447273a4fbce97bc8c515f97ac74efc461f6a4001553712f31ebc11288bad2");
        assert_eq!(Jovian::l1_block_proxy_update(), expected);
    }

    #[test]
    fn test_gas_price_oracle_source_hash() {
        let expected = b256!("d939cca6eca7bd0ee0c7e89f7e5b5cf7bf6f7afe7b6966bb45dfb95344b31545");
        assert_eq!(Jovian::gas_price_oracle(), expected);
    }

    #[test]
    fn test_gas_price_oracle_proxy_update_source_hash() {
        let expected = b256!("46b597e2d8346ed7749b46734074361e0b41a0ab9af7afda5bb4e367e072bcb8");
        assert_eq!(Jovian::gas_price_oracle_proxy_update(), expected);
    }

    #[test]
    fn test_gas_price_oracle_enable_jovian_source_hash() {
        let expected = b256!("e836db6a959371756f8941be3e962d000f7e12a32e49e2c9ca42ba177a92716c");
        assert_eq!(Jovian::gas_price_oracle_enable_jovian(), expected);
    }

    #[test]
    fn test_enable_jovian_source() {
        let expected = b256!("3ddf4b1302548dd92939826e970f260ba36167f4c25f18390a5e8b194b295319");
        assert_eq!(Jovian::enable_jovian_source(), expected);
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
