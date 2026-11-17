//! The build of a project with Rust POUs on the toolchain of specification 48 §16: it compiles and
//! links nothing for the computer itself (RU-74), so that a computer without Rust and without a
//! linker of its own builds the project as any other (RU-76).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`, as
//! `scripts/native/build_toolchain_libraries.sh` builds it. A toolchain without cargo and the SDK's
//! sources, as a release's bundle is until specification 48 §16, refuses the build with that
//! reason.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
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
    // RU-74, RU-09: the toolchain carries the SDK's procedural macros prebuilt for this computer and
    // the output of the SDK's build scripts (libm's) for each module target; the build gives the
    // compiler the macros and cargo the scripts' output, so the build of a project with Rust POUs
    // compiles and links nothing for the computer itself: cargo's folder of the computer's own
    // artifacts holds none, neither the SDK's macros with their build scripts nor libm's build
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
