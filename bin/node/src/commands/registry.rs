//! Registry Subcommand

use crate::flags::GlobalArgs;
use clap::Parser;

/// The `registry` Subcommand
///
/// The `registry` subcommand lists the OP Stack chains available in the `superchain-registry`.
///
/// # Usage
///
/// ```sh
/// kona-node registry [FLAGS] [OPTIONS]
/// ```
#[derive(Parser, Debug, Clone)]
#[command(about = "Lists the OP Stack chains available in the superchain-registry")]
pub struct RegistryCommand {}

impl RegistryCommand {
    /// Runs the subcommand.
    pub fn run(self, _args: &GlobalArgs) -> anyhow::Result<()> {
        let chains = kona_registry::CHAINS.chains.clone();
        let mut table = tabled::Table::new(chains);
        table.with(tabled::settings::Style::modern());
        table.modify(
            tabled::settings::object::Columns::first(),
            tabled::settings::Alignment::right(),
        );
        println!("{}", table);
        Ok(())
    }
}
