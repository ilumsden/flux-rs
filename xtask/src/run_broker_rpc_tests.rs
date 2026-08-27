use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use xshell::{Shell, cmd};

use crate::build_integration_tests::build_integration_tests;
use crate::flux_utils::{run_under_flux, start_flux_instance};
use crate::subcommand_args::TestBrokerRpcArgs;

fn run_broker_module_init_failure(sh: &Shell, broker_mod: &PathBuf) -> Result<()> {
    println!(
        "🚀 Step 2: Executing test for expected Flux broker module failures with `--init-failure`..."
    );

    let init_failure_output = cmd!(sh, "flux module load {broker_mod} --init-failure")
        .ignore_status()
        .output()?;

    println!("🔍 Step 3: Validating output...");

    if init_failure_output.status.success() {
        bail!(
            "Loading Rust broker module with `--init-failure` succeeded when it wasn't supposed to"
        );
    }

    let init_failure_dmesg = cmd!(sh, "flux dmesg").read()?;

    let init_failure_dmesg_lines: Vec<&str> = init_failure_dmesg
        .lines()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let init_failure_info_count = init_failure_dmesg_lines
        .iter()
        .filter(|line| {
            line.contains(
                "Triggering failure because the user requested to do so via command-line arguments",
            )
        })
        .count();

    if init_failure_info_count != 1 {
        bail!(
            "The Rust broker module did not produce the expected number out info messages regarding the --init-failure flag (got {}, expected 1)",
            init_failure_info_count
        );
    }

    let init_failure_err_count = init_failure_dmesg_lines
        .iter()
        .filter(|line| line.contains("Flux logic error: Aborting during init per test request"))
        .count();

    println!("Err count: {}", init_failure_err_count);

    if init_failure_err_count != 1 {
        bail!(
            "The Rust broker module did not produce the expected number out error messages regarding the --init-failure flag (got {}, expected 1)",
            init_failure_err_count
        );
    }

    cmd!(sh, "flux resource list")
        .quiet()
        .run()
        .context("Could not connect to the broker after crashing the Rust broker module with an Error return")?;

    println!("✅ Test for `--init-failure` passed successfully.");

    Ok(())
}

fn run_broker_module_init_panic(sh: &Shell, broker_mod: &PathBuf) -> Result<()> {
    println!(
        "🚀 Step 4: Executing test for expected Flux broker module failures with `--init-panic`..."
    );
    let init_panic_output = cmd!(sh, "flux module load {broker_mod} --init-panic")
        .ignore_status()
        .output()?;

    println!("🔍 Step 5: Validating output...");

    if init_panic_output.status.success() {
        bail!(
            "Loading Rust broker module with `--init-panic` succeeded when it wasn't supposed to"
        );
    }

    let init_panic_dmesg = cmd!(sh, "flux dmesg").read()?;

    let init_panic_dmesg_lines: Vec<&str> = init_panic_dmesg
        .lines()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let init_panic_err_count = init_panic_dmesg_lines
        .iter()
        .filter(|line| {
            line.contains("Panic occured in module main: Panicking during init per test request")
        })
        .count();

    if init_panic_err_count != 1 {
        bail!(
            "The Rust broker module did not produce the expected number out error messages regarding the --init-panic flag (got {}, expected 1)",
            init_panic_err_count
        );
    }

    cmd!(sh, "flux resource list").quiet().run().context(
        "Could not connect to the broker after crashing the Rust broker module with a panic",
    )?;

    println!("✅ Test for `--init-panic` passed successfully.");

    Ok(())
}

fn run_module_and_rpc(
    sh: &Shell,
    broker_mod: &PathBuf,
    rpc_client: &PathBuf,
    uname: &str,
    module_name: &str,
    service_name: &str,
) -> Result<()> {
    println!("🚀 Step 6: Executing test for Flux broker module and RPC client...");
    cmd!(
        sh,
        "flux module load --name={module_name} {broker_mod} --service {service_name}"
    )
    .quiet()
    .run()
    .context("Could not launch Rust broker module for testing RPC client")?;

    let rpc_client_output = cmd!(sh, "{rpc_client} {uname} {module_name} {service_name}")
        .ignore_status()
        .output()?;

    println!("🔍 Step 6: Validating output...");

    if !rpc_client_output.status.success() {
        bail!(
            "The RPC client failed with the following error:\n{}",
            String::from_utf8_lossy(&rpc_client_output.stderr)
        );
    }

    let expected_lines = vec![
        format!("Sending RPC to '{}.info' target", module_name),
        String::from("Getting service name from RPC"),
        String::from("Got expected service name from broker module!"),
        format!("Sending Streaming RPC to '{}.info' target", module_name),
        String::from("Getting service name from RPC"),
        String::from("Got expected service name from broker module!"),
        String::from("Checking for ENODATA to indicate end-of-stream"),
        String::from("Got expected ENODATA error to indicate end of streaming RPC!"),
        format!("Sending RPC to '{}.hello_world' target", module_name),
        String::from("Got expected response from the `hello_world` topic from the broker module!"),
    ];

    let stdout_lines: Vec<String> = String::from_utf8_lossy_owned(rpc_client_output.stdout)
        .lines()
        .map(String::from)
        .collect();

    if stdout_lines != expected_lines {
        bail!(
            "Did not get the expected output from the RPC client.\n\nExpected:\n{}\nGot:\n{}",
            expected_lines.join("\n"),
            stdout_lines.join("\n")
        )
    }

    let _ = cmd!(sh, "flux module remove {module_name}").quiet().run();

    println!("✅ Test for Flux broker module and RPC client passed successfully.");

    Ok(())
}

pub fn run_integration_tests(sh: &Shell, args: TestBrokerRpcArgs) -> Result<()> {
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
    let broker_mod = target_dir.join(format!("libflux_core_rs_test_broker_module.{}", ext));
    let rpc_client = target_dir.join("flux-core-rs-test-rpc");

    {
        let (_flux_instance_guard, flux_uri) = start_flux_instance(sh)?;

        run_under_flux!(sh, flux_uri, {
            run_broker_module_init_failure(sh, &broker_mod)?;

            run_broker_module_init_panic(sh, &broker_mod)?;

            run_module_and_rpc(
                sh,
                &broker_mod,
                &rpc_client,
                &args.uname,
                &args.broker_module_name,
                &args.service_name,
            )?;
        });
    }

    println!("✅ All broker module and RPC client tests passed successfully! ✅");
    Ok(())
}
