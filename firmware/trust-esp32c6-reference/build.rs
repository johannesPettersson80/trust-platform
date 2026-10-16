use std::{env, fs, path::PathBuf};

fn main() {
    // esp-hal includes `rwtext_hook.x` inside its RAM code section when
    // ESP_HAL_CONFIG_USE_RWTEXT_LD_HOOK is set (see .cargo/config.toml).
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    fs::copy("rwtext_hook.x", output.join("rwtext_hook.x")).expect("copy reviewed linker hook");
    println!("cargo:rerun-if-changed=rwtext_hook.x");
    println!("cargo:rustc-link-search={}", output.display());
}
