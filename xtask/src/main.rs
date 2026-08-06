use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use xshell::Shell;

mod build_integration_tests;
mod run_integration_tests;
mod subcommand_args;

use crate::build_integration_tests::build_integration_tests;
use crate::run_integration_tests::run_integration_tests;
use crate::subcommand_args::{BuildFixturesArgs, TestIntegrationArgs};

#[derive(Parser)]
#[command(name = "cargo xtask", about = "Pure-Rust Flux integration testing")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build test fixtures (broker module, jobtap plugin, RPC client)
    BuildFixtures(BuildFixturesArgs),
    /// Run full integration tests with Flux
    TestIntegration(TestIntegrationArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize the xshell environment
    let sh = Shell::new().context("Failed to initialize xshell")?;

    match cli.command {
        Commands::BuildFixtures(args) => build_integration_tests(&sh, args.release),
        Commands::TestIntegration(args) => run_integration_tests(&sh, args),
    }
}
