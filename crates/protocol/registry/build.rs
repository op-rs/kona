//! Build script that generates a `configs.json` file from the configs.

use kona_genesis::{ChainConfig, Superchain, SuperchainConfig, Superchains};

fn main() {
    // If the `KONA_BIND` environment variable is _not_ set, then return early.
    let kona_bind: bool =
        std::env::var("KONA_BIND").unwrap_or_else(|_| "false".to_string()) == "true";
    if !kona_bind {
        return;
    }

    // Get the directory of this file from the environment
    let src_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    // Check if the `superchain-registry` directory exists
    let superchain_registry = format!("{src_dir}/superchain-registry");
    if !std::path::Path::new(&superchain_registry).exists() {
        panic!("Git Submodule missing. Please run `just source` to initialize the submodule.");
    }

    // Copy the `superchain-registry/chainList.json` file to `etc/chainList.json`
    let chain_list = format!("{src_dir}/superchain-registry/chainList.json");
    let etc_dir = std::path::Path::new("etc");
    if !etc_dir.exists() {
        std::fs::create_dir_all(etc_dir).unwrap();
    }
    std::fs::copy(chain_list, "etc/chainList.json").unwrap();

    // Get the `superchain-registry/superchain/configs` directory`
    let configs_dir = format!("{src_dir}/superchain-registry/superchain/configs");
    let configs = std::fs::read_dir(configs_dir).unwrap();

    // Get all the directories in the `configs` directory
    let mut superchains = Superchains::default();
    for config in configs {
        let config = config.unwrap();
        let config_path = config.path();
        let superchain_name = config.file_name().into_string().unwrap();
        let mut superchain =
            Superchain { name: superchain_name, chains: Vec::new(), ..Default::default() };
        if config_path.is_dir() {
            let config_files = std::fs::read_dir(&config_path).unwrap();
            for config_file in config_files {
                let config_file = config_file.unwrap();
                let config_file_path = config_file.path();

                // Read the `superchain.toml` as the `SuperchainConfig`
                let config_file_name = config_file.file_name().into_string().unwrap();
                if config_file_name == "superchain.toml" {
                    let config = std::fs::read_to_string(config_file_path).unwrap();
                    let config: SuperchainConfig = toml::from_str(&config).unwrap();
                    superchain.config = config;
                    continue;
                }

                // Read the config file as a `ChainConfig`
                let config = std::fs::read_to_string(config_file_path).unwrap();
                let config: ChainConfig = toml::from_str(&config).unwrap();
                superchain.chains.push(config);
            }
            superchains.superchains.push(superchain);
        }
    }

    // Sort the superchains by name.
    superchains.superchains.sort_by(|a, b| a.name.cmp(&b.name));

    // For each superchain, sort the list of chains by chain id.
    for superchain in superchains.superchains.iter_mut() {
        superchain.chains.sort_by(|a, b| a.chain_id.cmp(&b.chain_id));
    }

    let output_path = std::path::Path::new("etc/configs.json");
    std::fs::write(output_path, serde_json::to_string_pretty(&superchains).unwrap()).unwrap();
}
