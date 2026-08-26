use std::cell::RefCell;
use std::env;

use semver::Version;

thread_local! {
    static KNOWN_CFGS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
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
        println!("cargo::rustc-check-cfg=cfg({joined})");
    });
}

fn add_core_conditional_compilation_checks() {
    let flux_core_version = env::var("DEP_FLUX_CORE_VERSION")
        .ok()
        .and_then(|v| Version::parse(&v).ok())
        .unwrap();

    if flux_core_version < Version::parse("0.71.0").unwrap() {
        panic!(
            "The `flux-core` Rust crate requires at least version 0.71.0 of libflux_core.so, but the flux-sys crate was built against version {}.{}.{}",
            flux_core_version.major, flux_core_version.minor, flux_core_version.patch
        );
    }

    let has_module_loader_helpers = flux_core_version >= Version::parse("0.83.1").unwrap();
    // Module loader helpers are first exported in v0.83.1
    create_conditional_compilation_var!(
        "flux_core_has_module_loader_helpers",
        has_module_loader_helpers
    );
}

fn main() {
    add_core_conditional_compilation_checks();
    declare_check_cfgs();
}
