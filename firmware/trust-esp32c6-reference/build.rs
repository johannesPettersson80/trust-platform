use std::{env, fs, path::PathBuf};

fn main() {
    // esp-hal includes `rwtext_hook.x` inside its RAM code section when
    // ESP_HAL_CONFIG_USE_RWTEXT_LD_HOOK is set (see .cargo/config.toml).
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    for name in ["rwtext_hook.x", "stack_guard.x"] {
        fs::copy(name, output.join(name)).expect("copy reviewed linker script");
        println!("cargo:rerun-if-changed={name}");
    }
    println!("cargo:rustc-link-search={}", output.display());
    // The stack guard is its own script: it asserts the region below `_stack_start` holds the
    // window plus its margin even when the RAM-code hook is not the reason the region shrank.
    let guard = output.join("stack_guard.x");
    println!("cargo:rustc-link-arg=-T{}", guard.display());
}
