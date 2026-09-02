use std::collections::HashMap;

use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::flux_utils::update_shell_env;
use crate::subcommand_args::CoverageArgs;

fn print_test_start(msg: &str) {
    let width = msg.chars().count() + 2;
    let border = "━".repeat(width);
    println!("\n┏{border}┓");
    println!("┃ {msg} ┃");
    println!("┗{border}┛");
}

fn print_test_end(msg: &str, res: Result<()>) -> Result<()> {
    let (icon, status) = if res.is_ok() {
        ("✔", "SUCCESS")
    } else {
        ("✖", "FAILED")
    };
    let end_msg = format!("{icon} {status}: {msg}");

    let width = end_msg.chars().count() + 2;
    let border = "━".repeat(width);
    // Pad the end message with spaces so the box width perfectly matches the header
    let padding = width.saturating_sub(end_msg.chars().count() + 2);
    let spaces = " ".repeat(padding);

    println!("┏{border}┓");
    println!("┃ {end_msg}{spaces} ┃");
    println!("┗{border}┛\n");

    res
}

pub fn run_code_coverage(sh: &Shell, args: CoverageArgs) -> Result<()> {
    cmd!(sh, "cargo llvm-cov clean --workspace").run()?;

    let cov_env_output = cmd!(sh, "cargo llvm-cov show-env").read()?;

    let env_map: HashMap<String, String> = cov_env_output
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            (
                key.trim_start_matches("export ").trim().to_string(),
                value.trim_matches(|c| c == '"' || c == '\'').to_string(),
            )
        })
        .collect();

    let _env_updates = update_shell_env(sh, &env_map);

    print_test_start("Unit Test Coverage");
    let res = if args.release {
        cmd!(
            sh,
            "flux start cargo test --all-features --release" // "flux start cargo llvm-cov test --no-report --all-features --release"
        )
        .run()
        .context("Unit tests failed")
    } else {
        cmd!(
            sh,
            "flux start cargo test --all-features" //"flux start cargo llvm-cov test --no-report --all-features"
        )
        .run()
        .context("Unit tests failed")
    };
    print_test_end("Unit Test Coverage", res)?;

    print_test_start("Broker Module/RPC Integration Test Coverage");
    let res = if args.release {
        cmd!(sh, "cargo xtask test-broker-rpc --release")
            .run()
            .context("Broker Module/RPC Integration Test Failed")
    } else {
        cmd!(sh, "cargo xtask test-broker-rpc")
            .run()
            .context("Broker Module/RPC Integration Test Failed")
    };
    print_test_end("Broker Module/RPC Integration Test Coverage", res)?;

    print_test_start("LCOV Report Generation");
    let res = cmd!(
        sh,
        "cargo llvm-cov report --package flux-core --lcov --output-path lcov.info"
    )
    .run()
    .context("Failed to generate LCOV report");
    print_test_end("LCOV Report Generation", res)?;

    print_test_start("HTML Report Generation");
    let res = cmd!(
        sh,
        "cargo llvm-cov report --package flux-core --html --output-dir coverage-html"
    )
    .run()
    .context("Failed to generate HTML report");
    print_test_end("HTML Report Generation", res)?;

    Ok(())
}
