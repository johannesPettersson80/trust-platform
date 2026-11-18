//! The build of a project with Rust POUs on the toolchain of specification 48 §16: it compiles and
//! links nothing for the computer itself (RU-74), so that a computer without Rust and without a
//! linker of its own builds the project as any other (RU-76).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`, as
//! `scripts/native/build_toolchain_libraries.sh` builds it. A toolchain without cargo and the SDK's
//! sources, as a release's bundle is until specification 48 §16, refuses the build with that
//! reason.

mod rust_pous_support;

use std::path::Path;
use std::process::Command;

use rust_pous_support::{build_error, built, custom_project, manifest};
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
    // RU-75: the build gives its cargo the toolchain's own home and none of the user's cargo
    // settings from the environment, so the user's cargo home, its configuration and the
    // environment's play no part. `trust-runtime build` runs as a user whose cargo configuration
    // cargo cannot read, at `CARGO_HOME` and at `~/.cargo`, and whose environment holds settings
    // that cargo refuses, `CARGO_BUILD_JOBS=0` and a release profile's `opt-level` of `fast`, and a
    // compiler wrapper that does not exist; the build builds the project as before, and the user's
    // cargo home gains no registry. Catches a build whose cargo runs in the user's home, which
    // would take the user's configuration (a mirror for crates.io, a target folder, flags) and fill
    // the user's cache, and one whose cargo takes the environment's settings, which change what it
    // builds (a release profile's overflow checks) and how it reports.
    let toolchain = Toolchain::find().expect("the toolchain");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let root = custom_project(
        "rust-pous-own-cargo-home",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", CRATE)],
        PROGRAM,
    );
    write_runtime_configuration(&root);
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
    .env("CARGO_BUILD_JOBS", "0")
    .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "fast")
    .env("RUSTC_WRAPPER", user.join("no-such-wrapper"))
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

#[test]
fn a_failed_build_reports_the_projects_error_without_the_vendored_crates_warnings() {
    // RU-44 and RU-74: the build reports the diagnostics of the project's crate. The crates.io
    // crates that the toolchain vendors are path dependencies of the build, whose lints cargo does
    // not cap as it caps a registry crate's, so each copy's manifest allows the crate's warnings.
    // A crate with a type error fails the build, whose report names the error, E0308, and holds no
    // diagnostic of a vendored crate: none located in its folder, none of cargo's counts of its
    // warnings. Catches the report before the copies allowed them: libm's 18 warnings with the
    // pinned compiler, about 280 lines before the project's error, with cargo's advice to run
    // `cargo fix -p libm`, on every failed build.
    let root = custom_project(
        "rust-pous-report-without-vendored-warnings",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &crate_with_a_type_error()),
        ],
        PROGRAM,
    );
    let (error, toolchain) = build_error(&root);
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        assert!(error.contains("no cargo and SDK sources"), "{error}");
        return;
    }
    assert!(error.contains("error[E0308]: mismatched types"), "{error}");
    let vendor = toolchain.vendor.expect("the toolchain's vendored crates");
    let crates: Vec<String> = std::fs::read_dir(&vendor)
        .expect("the vendored crates' folder")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(crates.iter().any(|name| name == "libm"), "{crates:?}");
    let vendored: Vec<&str> = error
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.strip_prefix("--> ")
                .is_some_and(|place| place.starts_with("vendor/") || place.starts_with("vendor\\"))
                || crates
                    .iter()
                    .any(|name| line.starts_with(&format!("warning: `{name}` ")))
        })
        .collect();
    assert!(vendored.is_empty(), "{vendored:#?}\n{error}");
}

#[test]
fn a_failed_builds_report_is_plain_text_whatever_colour_the_users_environment_asks() {
    // RU-44 and RU-75: the build reads its cargo's report and passes it on as text, to a terminal,
    // to VS Code or to a program, so its cargo writes the report without colour, whatever the
    // user's environment asks of cargo. `trust-runtime build` of a crate with a type error runs
    // where `CARGO_TERM_COLOR=always` asks cargo for colour, as CI's environment does; the report
    // names the error, E0308, in plain text and holds no escape code. Catches a build whose cargo
    // takes the colour from the user's environment: on CI the report read
    // `ESC[1mESC[91merror[E0308]ESC[0mESC[1m: mismatched types` (#604, 18 November 2026), which
    // neither a person nor the build's own filter of the report reads as the error.
    let toolchain = Toolchain::find().expect("the toolchain");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let root = custom_project(
        "rust-pous-report-in-plain-text",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &crate_with_a_type_error()),
        ],
        PROGRAM,
    );
    write_runtime_configuration(&root);
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo provides the trust-runtime binary to integration tests"),
    )
    .args(["build", "--project"])
    .arg(&root)
    .env("CARGO_TERM_COLOR", "always")
    .output()
    .expect("run trust-runtime");
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "the build fails:\n{report}");
    assert!(
        report.contains("error[E0308]: mismatched types"),
        "the report names the error in plain text:\n{report}"
    );
    assert!(
        !report.contains('\u{1b}'),
        "the report holds no escape code: {report:?}"
    );
}

#[test]
fn a_failed_build_names_each_error_of_the_crate_where_it_is() {
    // RU-44 and specification 11 §6.8.1d: an error of the project's crate is reported with its
    // `.rs` file, line, column and message. The type error of `crate_with_a_type_error` is line 10
    // of `rust/src/lib.rs`, `    let wrong: u8 = "text";`, whose `"text"` starts after 20
    // characters and is 6 long: columns 21 to 27. `trust-runtime build` reports it as the compiler
    // writes it, at the file as the project names it; with `--json` the failure's issues hold that
    // one error, from the source `truST (rustc)`, with the compiler's code. Catches a report or an
    // issue at the compiler's remapped `project/src/lib.rs`, an error with no place, and a build
    // that gives a Rust error as one text without issues.
    let toolchain = Toolchain::find().expect("the toolchain");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let root = custom_project(
        "rust-pous-error-where-it-is",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &crate_with_a_type_error()),
        ],
        PROGRAM,
    );
    write_runtime_configuration(&root);
    let human = cli_build(&root, &[]);
    let report = String::from_utf8_lossy(&human.stderr);
    assert!(!human.status.success(), "the build fails:\n{report}");
    assert!(
        report.contains("error[E0308]: mismatched types\n  --> rust/src/lib.rs:10:21\n"),
        "{report}"
    );
    assert!(!report.contains("project/src/lib.rs"), "{report}");
    let json = cli_build(&root, &["--json"]);
    assert!(!json.status.success());
    let failure: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("the failure as JSON");
    assert_eq!(failure["status"], "failed", "{failure:#}");
    assert_eq!(failure["errors"], 1, "{failure:#}");
    assert_eq!(
        failure["issues"],
        serde_json::json!([{
            "severity": "error",
            "message": "mismatched types",
            "code": "E0308",
            "source": "truST (rustc)",
            "file": "rust/src/lib.rs",
            "line": 10,
            "column": 21,
            "end_line": 10,
            "end_column": 27,
        }])
    );
}

#[test]
fn a_crate_warning_is_reported_where_it_is_and_does_not_fail_the_build() {
    // RU-44: a warning of the project's crate is reported with its place and does not fail the
    // build. The crate's line 10, `    let spare = 1;`, leaves `spare` unused: the compiler's
    // `unused_variables` at columns 9 to 14 of `rust/src/lib.rs`, the 5 characters after `let `.
    // `trust-runtime build` builds the image and writes the warning as the compiler writes it;
    // with `--json` the success has one warning issue. Catches a warning that fails the build, one
    // a successful build keeps to itself, as every build did before, and the generated module's
    // own warnings (an unused import of `module.rs`) reported as the crate's.
    let toolchain = Toolchain::find().expect("the toolchain");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let source = CRATE.replace(
        "    Ok(a.wrapping_add(1))",
        "    let spare = 1;\n    Ok(a.wrapping_add(1))",
    );
    let root = custom_project(
        "rust-pous-warning-where-it-is",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", &source)],
        PROGRAM,
    );
    write_runtime_configuration(&root);
    let human = cli_build(&root, &[]);
    let report = String::from_utf8_lossy(&human.stderr);
    assert!(human.status.success(), "the build succeeds:\n{report}");
    assert!(root.join("program.tpi").is_file());
    assert!(
        report.contains("warning: unused variable: `spare`\n  --> rust/src/lib.rs:10:9\n"),
        "{report}"
    );
    assert!(!report.contains("module.rs"), "{report}");
    let json = cli_build(&root, &["--json"]);
    assert!(json.status.success());
    let built: serde_json::Value = serde_json::from_slice(&json.stdout).expect("the build as JSON");
    assert_eq!(built["status"], "ok", "{built:#}");
    assert_eq!(built["warnings"], 1, "{built:#}");
    assert_eq!(built["issues"][0]["severity"], "warning", "{built:#}");
    assert_eq!(built["issues"][0]["code"], "unused_variables");
    assert_eq!(built["issues"][0]["source"], "truST (rustc)");
    assert_eq!(built["issues"][0]["file"], "rust/src/lib.rs");
    assert_eq!(
        [
            &built["issues"][0]["line"],
            &built["issues"][0]["column"],
            &built["issues"][0]["end_line"],
            &built["issues"][0]["end_column"],
        ],
        [10, 9, 10, 14]
    );
}

/// `trust-runtime build` of the project at `root` with the arguments `args`.
fn cli_build(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo provides the trust-runtime binary to integration tests"),
    )
    .args(["build", "--project"])
    .arg(root)
    .args(args)
    .output()
    .expect("run trust-runtime")
}

/// `CRATE` with a type error in its function: a `&str` given to a `u8` (E0308).
fn crate_with_a_type_error() -> String {
    CRATE.replace(
        "    Ok(a.wrapping_add(1))",
        "    let wrong: u8 = \"text\";\n    Ok(a.wrapping_add(1))",
    )
}

/// Writes the runtime's configuration and the loopback I/O of the project at `root`, which
/// `trust-runtime build` builds for this computer.
fn write_runtime_configuration(root: &Path) {
    let runtime = render_runtime_toml(&SmolStr::new("probe"), 10);
    // Windows has no Unix sockets: the control link listens on TCP there, with a token.
    #[cfg(windows)]
    let runtime = runtime.replacen(
        "[runtime.control]\n",
        "[runtime.control]\nendpoint = \"tcp://127.0.0.1:19095\"\nauth_token = \"build-probe-token\"\n",
        1,
    );
    std::fs::write(root.join("runtime.toml"), runtime).expect("runtime.toml");
    let io = render_io_toml(&build_io_config_auto("loopback").expect("loopback I/O"));
    std::fs::write(root.join("io.toml"), io).expect("io.toml");
}
