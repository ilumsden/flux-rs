use std::cell::RefCell;
use std::env;

use semver::Version;

thread_local! {
    static KNOWN_CFGS: RefCell<Vec<&'static str>> = RefCell::new(Vec::new());
}

macro_rules! create_conditional_compilation_var {
    ($var_name:expr) => {{
        KNOWN_CFGS.with(|cfgs| cfgs.borrow_mut().push($var_name));
        println!("cargo:rustc-cfg={}", $var_name);
    }};
    ($var_name:expr, $condition:expr) => {{
        KNOWN_CFGS.with(|cfgs| cfgs.borrow_mut().push($var_name));
        if $condition {
            println!("cargo:rustc-cfg={}", $var_name);
        }
    }};
}

fn declare_check_cfgs() {
    KNOWN_CFGS.with(|cfgs| {
        let known = cfgs.borrow();
        if known.is_empty() {
            return;
        }
        let joined = known.join(", ");
        println!("cargo::rustc-check-cfg=cfg({})", joined);
    });
}

fn add_core_conditional_compilation_checks() {
    let has_module_loader_helpers = env::var("DEP_FLUX_CORE_VERSION")
        .ok()
        .and_then(|v| Version::parse(&v).ok())
        .map(|v| v >= Version::parse("0.83.1").unwrap())
        .unwrap_or(false);
    let has_proper_reactor_ref_count = env::var("DEP_FLUX_CORE_VERSION")
        .ok()
        .and_then(|v| Version::parse(&v).ok())
        .map(|v| v >= Version::parse("0.70.0").unwrap())
        .unwrap_or(false);
    create_conditional_compilation_var!(
        "flux_core_has_module_loader_helpers",
        has_module_loader_helpers
    );
    create_conditional_compilation_var!(
        "flux_core_has_reactor_ref_count",
        has_proper_reactor_ref_count
    );
}

fn main() {
    add_core_conditional_compilation_checks();
    declare_check_cfgs();
}
