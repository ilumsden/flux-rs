use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::build_integration_tests::build_integration_tests;
use crate::subcommand_args::TestIntegrationArgs;

pub fn run_integration_tests(sh: &Shell, args: TestIntegrationArgs) -> Result<()> {
    println!("🧪 Step 1: Building test fixtures...");
    build_integration_tests(sh, args.release)?;

    let profile = if args.release { "release" } else { "debug" };

    // sh.current_dir() points to the workspace root when run via `cargo xtask`
    let target_dir = sh.current_dir().join("target").join(profile);

    #[cfg(target_os = "macos")]
    let ext = "dylib";
    #[cfg(not(target_os = "macos"))]
    let ext = "so";

    // xshell macros can interpolate PathBufs directly
    let broker_mod = target_dir.join(format!("libtest_broker_mod.{}", ext));
    let jobtap_plugin = target_dir.join(format!("libtest_jobtap_plugin.{}", ext));
    let rpc_client = target_dir.join("rpc-client");

    // Set environment variables directly on the shell state
    sh.set_var("JOBTAP_PLUGIN_PATH", &jobtap_plugin);
    if args.skip_teardown {
        sh.set_var("FLUX_SKIP_TEARDOWN", "1");
    }

    println!("🚀 Step 2: Executing Flux integration test...");

    // .read() runs the command, asserts a zero exit code, and returns stdout as a String
    let output = cmd!(
        sh,
        "flux start --setattr=rc3.module.load.test_broker={broker_mod} {rpc_client}"
    )
    .read()
    .context("Flux execution failed or returned a non-zero exit code")?;

    println!("🔍 Step 3: Validating output...");
    if !output.contains("Expected RPC Response") {
        anyhow::bail!(
            "Integration test failed! Could not find expected output.\n\nSTDOUT:\n{}",
            output
        );
    }

    println!("✅ Integration tests passed successfully.");
    Ok(())
}
