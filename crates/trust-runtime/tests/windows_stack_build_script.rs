use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_build_script_binary() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "trust-runtime-build-script-{}-{nanos}-{sequence}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ))
}

fn run_for_target(binary: &PathBuf, target_os: &str, target_env: &str) -> Output {
    Command::new(binary)
        .env_clear()
        .env("CARGO_CFG_TARGET_OS", target_os)
        .env("CARGO_CFG_TARGET_ENV", target_env)
        .output()
        .expect("run compiled trust-runtime build script")
}

#[test]
fn windows_stack_link_arguments_match_toolchain_and_other_targets_emit_none() {
    let binary = unique_build_script_binary();
    let build_script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("build.rs");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let compile = Command::new(rustc)
        .arg("--edition=2021")
        .arg(&build_script)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile trust-runtime build script");
    assert!(
        compile.status.success(),
        "build script compilation failed:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let linux = run_for_target(&binary, "linux", "gnu");
    assert!(linux.status.success());
    assert_eq!(
        String::from_utf8_lossy(&linux.stdout),
        "cargo:rerun-if-changed=build.rs\n"
    );

    let msvc = run_for_target(&binary, "windows", "msvc");
    assert!(msvc.status.success());
    assert_eq!(
        String::from_utf8_lossy(&msvc.stdout),
        concat!(
            "cargo:rerun-if-changed=build.rs\n",
            "cargo:rustc-link-arg-bin=trust-runtime=/STACK:8388608\n"
        )
    );

    let gnu = run_for_target(&binary, "windows", "gnu");
    assert!(gnu.status.success());
    assert_eq!(
        String::from_utf8_lossy(&gnu.stdout),
        concat!(
            "cargo:rerun-if-changed=build.rs\n",
            "cargo:rustc-link-arg-bin=trust-runtime=-Wl,--stack,8388608\n"
        )
    );

    std::fs::remove_file(binary).expect("remove compiled build script fixture");
}
