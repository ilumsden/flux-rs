use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use xshell::{PushEnv, Shell};

pub struct FluxInstanceGuard(std::process::Child);

impl Drop for FluxInstanceGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn start_flux_instance(sh: &Shell) -> Result<(FluxInstanceGuard, String)> {
    let mut child_proc = Command::new("flux")
        .args(["start", "sh", "-c", "flux uri --local . && sleep inf"])
        .current_dir(sh.current_dir())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn Flux instance in the background")?;
    thread::sleep(Duration::from_millis(500));
    let stdout = child_proc
        .stdout
        .take()
        .context("Failed to open stdout for the Flux instance spawning command")?;
    let mut reader = BufReader::new(stdout);
    let mut flux_uri = String::new();
    reader
        .read_line(&mut flux_uri)
        .context("Failed to read Flux URI from stdout")?;
    let flux_uri = flux_uri.trim().to_string();
    Ok((FluxInstanceGuard(child_proc), flux_uri))
}

pub fn update_shell_env<'a>(sh: &'a Shell, env_mods: &HashMap<String, String>) -> Vec<PushEnv<'a>> {
    env_mods.iter().map(|(k, v)| sh.push_env(k, v)).collect()
}

macro_rules! run_under_flux {
    ($sh:expr, $uri:expr, { $($body:tt)* }) => {{
        let _flux_uri_guard = $sh.push_env("FLUX_URI", $uri);
        $($body)*
    }};
    ($sh:expr, $uri:expr, env_mods:expr, { $($body:tt)* }) => {{
        let _env_mod_guards = $crate::flux_utils::update_shell_env($sh, $env_mods);
        $crate::flux_utils::run_under_flux!($sh, $uri, { $($body:tt)* })
    }};
}

pub(crate) use run_under_flux;
