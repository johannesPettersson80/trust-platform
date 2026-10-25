use std::{env, fs, path::PathBuf};

fn main() {
    // esp-hal includes `rwtext_hook.x` inside its RAM code section when
    // ESP_HAL_CONFIG_USE_RWTEXT_LD_HOOK is set (see .cargo/config.toml).
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    for name in ["stack_guard.x", "trust_code.x"] {
        fs::copy(name, output.join(name)).expect("copy reviewed linker script");
        println!("cargo:rerun-if-changed={name}");
    }
    // The RAM code list (specification 44 §10.3) lies in `ld/`, not next to the manifest, and is
    // copied to the search path: the linker looks for `rwtext_hook.x` in its working directory first.
    fs::copy("ld/rwtext_hook.x", output.join("rwtext_hook.x"))
        .expect("copy reviewed linker script");
    println!("cargo:rerun-if-changed=ld/rwtext_hook.x");
    println!("cargo:rustc-link-search={}", output.display());
    // The stack guard is its own script: it asserts the region below `_stack_start` holds the
    // window plus its margin even when the RAM-code hook is not the reason the region shrank.
    let guard = output.join("stack_guard.x");
    println!("cargo:rustc-link-arg=-T{}", guard.display());
    // The program code region, between the static data and the stack (specification 44 §10.3).
    let code = output.join("trust_code.x");
    println!("cargo:rustc-link-arg=-T{}", code.display());
}
