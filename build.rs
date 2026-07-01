use std::env;

use semver::Version;

macro_rules! create_conditional_compilation_var {
    ($var_name:expr) => {
        println!("cargo:rustc-cfg={}", $var_name)
    };
}

fn add_core_conditional_compilation_checks() {
    if let Ok(flux_core_version) = env::var("DEP_FLUX_CORE_VERSION") {
        if let Ok(version) = Version::parse(&flux_core_version) {
            if version >= Version::parse("0.83.1").unwrap() {
                create_conditional_compilation_var!("flux_core_has_module_loader_helpers");
            }
        }
    }
}

fn main() {
    add_core_conditional_compilation_checks();
}
