use std::{env, fs, path::PathBuf};

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    for name in ["memory.x", "profile.x"] {
        fs::copy(name, output.join(name)).expect("copy reviewed linker script");
        println!("cargo:rerun-if-changed={name}");
    }
    println!("cargo:rustc-link-search={}", output.display());
}
