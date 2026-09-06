use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::subcommand_args::SetupPreCommitArgs;
use crate::ui::{print_test_end, print_test_start};
use crate::update_deny_db::run_update_deny_db;

fn tool_available(sh: &Shell, bin: &str) -> bool {
    cmd!(sh, "{bin} --version")
        .quiet()
        .ignore_stdout()
        .ignore_stderr()
        .run()
        .is_ok()
}

/// Returns whichever pre-commit-compatible binary is available, preferring
/// `prek` over `pre-commit` since it's Rust-native and faster.
fn resolve_hook_runner(sh: &Shell) -> Option<&'static str> {
    if tool_available(sh, "prek") {
        Some("prek")
    } else if tool_available(sh, "pre-commit") {
        Some("pre-commit")
    } else {
        None
    }
}

pub fn run_setup_pre_commit(sh: &Shell, args: SetupPreCommitArgs) -> Result<()> {
    print_test_start("Rustup Components");
    let res = cmd!(sh, "rustup component add rustfmt clippy")
        .run()
        .context("Failed to install rustup components");
    print_test_end("Rustup Components", res)?;

    print_test_start("Cargo Tools (cargo-deny, cargo-machete)");
    let res = cmd!(sh, "cargo install --locked cargo-deny cargo-machete")
        .run()
        .context("Failed to install cargo-deny / cargo-machete");
    print_test_end("Cargo Tools (cargo-deny, cargo-machete)", res)?;

    let mut runner = resolve_hook_runner(sh);

    if runner.is_none() && !args.skip_pre_commit_install {
        print_test_start("prek Installation");
        let res = cmd!(sh, "cargo install --locked prek")
            .run()
            .context("Failed to install prek (pre-commit compatible, Rust-native)");
        print_test_end("prek Installation", res)?;
        runner = Some("prek");
    }

    if let Some(runner) = runner {
        print_test_start("Git Hooks");
        let res = cmd!(
            sh,
            "{runner} install --hook-type pre-commit --hook-type pre-push"
        )
        .run()
        .context("Failed to install git hooks");
        print_test_end("Git Hooks", res)?;
    } else {
        println!(
            "\nSkipped git hook installation: neither `prek` nor `pre-commit` is available \
             (--skip-pre-commit-install was set and neither was already installed)."
        );
    }

    print_test_start("cargo-deny Advisory Database");
    let res = run_update_deny_db(sh).context("Failed to fetch advisory database");
    print_test_end("cargo-deny Advisory Database", res)?;

    println!("\nSetup complete. Try: prek run --all-files  (or: pre-commit run --all-files)");
    Ok(())
}
