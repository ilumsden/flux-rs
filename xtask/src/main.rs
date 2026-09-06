use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use xshell::Shell;

mod build_integration_tests;
mod coverage;
mod flux_utils;
mod run_broker_rpc_tests;
mod setup_pre_commit;
mod subcommand_args;
mod ui;
mod update_deny_db;

use crate::build_integration_tests::build_integration_tests;
use crate::coverage::run_code_coverage;
use crate::run_broker_rpc_tests::run_integration_tests;
use crate::setup_pre_commit::run_setup_pre_commit;
use crate::subcommand_args::{
    BuildFixturesArgs, CoverageArgs, SetupPreCommitArgs, TestBrokerRpcArgs, UpdateDenyDbArgs,
};
use crate::update_deny_db::run_update_deny_db_cmd;

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
    TestBrokerRpc(TestBrokerRpcArgs),
    /// Run code coverage across both unit and integration tests
    Coverage(CoverageArgs),
    /// Install everything needed for pre-commit
    SetupPreCommit(SetupPreCommitArgs),
    /// Update the database for `cargo-deny`
    UpdateDenyDb(UpdateDenyDbArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize the xshell environment
    let sh = Shell::new().context("Failed to initialize xshell")?;

    match cli.command {
        Commands::BuildFixtures(args) => build_integration_tests(&sh, args.release),
        Commands::TestBrokerRpc(args) => run_integration_tests(&sh, args),
        Commands::Coverage(args) => run_code_coverage(&sh, args),
        Commands::SetupPreCommit(args) => run_setup_pre_commit(&sh, args),
        Commands::UpdateDenyDb(args) => run_update_deny_db_cmd(&sh, args),
    }
}
