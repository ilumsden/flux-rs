use anyhow::{Context, Result};
use xshell::{Shell, cmd};

pub fn build_integration_tests(sh: &Shell, release: bool) -> Result<()> {
    // xshell lets us cleanly build an argument list
    let mut args = vec![
        "build",
        "--package",
        "test-broker-mod",
        "--package",
        "test-jobtap-plugin",
        "--package",
        "rpc-client",
    ];

    if release {
        args.push("--release");
    }

    // The cmd! macro interpolates iterables automatically with {args...}
    cmd!(sh, "cargo {args...}")
        .run()
        .context("Failed to compile workspace fixtures")?;

    Ok(())
}
