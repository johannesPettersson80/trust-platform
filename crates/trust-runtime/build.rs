fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "msvc" {
        println!("cargo:rustc-link-arg-bin=trust-runtime=/STACK:8388608");
    } else {
        println!("cargo:rustc-link-arg-bin=trust-runtime=-Wl,--stack,8388608");
    }
}
