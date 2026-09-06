use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::subcommand_args::UpdateDenyDbArgs;
use crate::ui::{print_test_end, print_test_start};

pub fn run_update_deny_db(sh: &Shell) -> Result<()> {
    cmd!(sh, "cargo deny fetch")
        .run()
        .context("cargo deny fetch failed")
}

pub fn run_update_deny_db_cmd(sh: &Shell, _args: UpdateDenyDbArgs) -> Result<()> {
    print_test_start("cargo-deny Advisory Database Update");
    let res = run_update_deny_db(sh);
    print_test_end("cargo-deny Advisory Database Update", res)?;
    Ok(())
}
