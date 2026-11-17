//! The build of a project with Rust POUs on the toolchain of specification 48 §16: it compiles and
//! links nothing for the computer itself (RU-74), so that a computer without Rust and without a
//! linker of its own builds the project as any other (RU-76).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`, as
//! `scripts/native/build_toolchain_libraries.sh` builds it. A toolchain without cargo and the SDK's
//! sources, as a release's bundle is until specification 48 §16, refuses the build with that
//! reason.

mod rust_pous_support;

use std::process::Command;

use rust_pous_support::{built, custom_project, manifest};
use smol_str::SmolStr;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};
use trust_runtime::native::{load, NoHooks, Toolchain};
use trust_runtime::value::{Duration, Value};

/// A Rust function, which the SDK's procedural macros declare.
const CRATE: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Its input plus one.
#[trust_function(name = \"PLUS_ONE\")]
#[budget(\"1us\")]
pub fn plus_one(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {
    Ok(a.wrapping_add(1))
}
";

/// A program that calls the Rust function and the program library's square root, which libm
/// computes.
const PROGRAM: &str = "PROGRAM Line
VAR
    bumped : DINT;
    root : LREAL;
END_VAR
bumped := PLUS_ONE(41);
root := SQRT(LREAL#2.25);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_project_with_rust_pous_builds_without_compiling_for_the_computer() {
    // RU-74, RU-09: the toolchain carries the SDK's procedural macros prebuilt for this computer
    // and the output of the SDK's build scripts (libm's) for each module target; the build gives
    // the compiler the macros and cargo the scripts' output, so the build of a project with Rust
    // POUs compiles and links nothing for the computer itself: cargo's folder of the computer's
    // own artifacts holds none, neither the SDK's macros with their build scripts nor libm's build
    // script. Catches a build that compiles either for the computer, which needs the computer's own
    // linker, which a computer without Rust may lack (macOS without Xcode's command-line tools has
    // none). The module then runs: the task is not due at 0 (specification 10 §4.3), so the scan at
    // 10 ms calls PLUS_ONE(41) = 42 and SQRT(2.25) = 1.5, exact in binary, through the libm that
    // the recorded output configured.
    let root = custom_project(
        "rust-pous-no-host-build",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", CRATE)],
        PROGRAM,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let toolchain = Toolchain::find().expect("the toolchain");
    assert!(
        toolchain.macros.is_some() && toolchain.vendor.is_some(),
        "the toolchain carries the SDK's macros prebuilt and its build scripts' output \
         (scripts/native/build_toolchain_libraries.sh)"
    );
    let computer = root.join("build/native/target/release");
    let built_for_the_computer: Vec<String> = ["deps", "build"]
        .iter()
        .filter_map(|folder| std::fs::read_dir(computer.join(folder)).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        built_for_the_computer.is_empty(),
        "nothing is built for the computer: {built_for_the_computer:?}"
    );
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in [0, 10] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("MAIN.BUMPED"), Some(Value::DInt(42)));
    assert_eq!(program.value("MAIN.ROOT"), Some(Value::LReal(1.5)));
}

#[test]
fn the_build_runs_cargo_in_a_home_of_its_own() {
    // RU-75: the build gives its cargo the toolchain's own home, so the user's cargo home and its
    // configuration play no part. `trust-runtime build` runs as a user whose cargo configuration
    // cargo cannot read, at `CARGO_HOME` and at `~/.cargo`, and builds the project as before; the
    // user's cargo home gains no registry. Catches a build whose cargo runs in the user's home,
    // which would take the user's configuration (a mirror for crates.io, a target folder, flags)
    // and fill the user's cache.
    let toolchain = Toolchain::find().expect("the toolchain");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let root = custom_project(
        "rust-pous-own-cargo-home",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", CRATE)],
        PROGRAM,
    );
    let runtime = render_runtime_toml(&SmolStr::new("probe"), 10);
    // Windows has no Unix sockets: the control link listens on TCP there, with a token.
    #[cfg(windows)]
    let runtime = runtime.replacen(
        "[runtime.control]\n",
        "[runtime.control]\nendpoint = \"tcp://127.0.0.1:19095\"\nauth_token = \"own-cargo-home-token\"\n",
        1,
    );
    std::fs::write(root.join("runtime.toml"), runtime).expect("runtime.toml");
    let io = render_io_toml(&build_io_config_auto("loopback").expect("loopback I/O"));
    std::fs::write(root.join("io.toml"), io).expect("io.toml");
    let user = root.join("user");
    let cargo_home = user.join("cargo");
    let unreadable = "this is not a cargo configuration [\n";
    for folder in [cargo_home.clone(), user.join("home/.cargo")] {
        std::fs::create_dir_all(&folder).expect("the user's cargo folder");
        std::fs::write(folder.join("config.toml"), unreadable).expect("the user's configuration");
    }
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo provides the trust-runtime binary to integration tests"),
    )
    .args(["build", "--project"])
    .arg(&root)
    .arg("--ci")
    .env("CARGO_HOME", &cargo_home)
    .env("HOME", user.join("home"))
    .env("XDG_CONFIG_HOME", user.join("config"))
    .env("XDG_DATA_HOME", user.join("data"))
    .output()
    .expect("run trust-runtime");
    assert!(
        output.status.success(),
        "status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        root.join("program.tpi").is_file(),
        "the build wrote no program image"
    );
    assert!(
        !cargo_home.join("registry").exists(),
        "the build filled the user's cargo cache"
    );
}
