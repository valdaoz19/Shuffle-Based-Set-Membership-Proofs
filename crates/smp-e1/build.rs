use std::{env, process::Command};
fn main() {
    for key in [
        "TARGET",
        "PROFILE",
        "OPT_LEVEL",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_CFG_TARGET_FEATURE",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
        let value = env::var(key).unwrap_or_default().replace('\u{1f}', " ");
        println!("cargo:rustc-env=SMP_BUILD_{key}={value}");
    }
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = Command::new(rustc)
        .arg("-Vv")
        .output()
        .expect("compiler metadata");
    println!(
        "cargo:rustc-env=SMP_BUILD_RUSTC={}",
        String::from_utf8_lossy(&version.stdout).replace('\n', "; ")
    );
}
