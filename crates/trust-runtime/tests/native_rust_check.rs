//! `trust-runtime check` checks a project with Rust POUs as the build checks it (specification 11
//! §6.8.1c): its Structured Text generated with the crate, and the crate with the generated module
//! by the toolchain's `cargo check`, its errors and warnings as the build's issues (specification 48
//! RU-44). Before, the check generated the program without the crate and refused every Rust POU,
//! so VS Code's Compile failed on every project with Rust POUs (checklist G208).
//!
//! The tests that check a crate need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`;
//! with a toolchain without cargo and the SDK's sources they end at the check's refusal.

mod rust_pous_support;

use std::path::Path;
use std::process::Command;

use rust_pous_support::{custom_project, manifest, sources};
use trust_runtime::bundle_builder::check_program;
use trust_runtime::native::{
    check_project, NativeError, Options, RustDiagnostic, RustSeverity, Target, Toolchain,
};
use trust_runtime::stdlib::conversions::ConversionProfile;

/// A crate whose function QUICK adds one, with `body` in its body before it returns.
fn crate_with(body: &str) -> String {
    format!(
        "#![cfg_attr(not(test), no_std)]\n#![forbid(unsafe_code)]\n\nuse trust_plc::prelude::*;\n\n\
         /// One more than `a`.\n#[trust_function(name = \"QUICK\")]\n#[budget(\"1ms\")]\n\
         pub fn quick(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {{\n{body}    Ok(a.wrapping_add(1))\n}}\n"
    )
}

/// The program that calls QUICK in a task.
const PROGRAM: &str = "PROGRAM Line
VAR
    next : DINT;
END_VAR
next := QUICK(next);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// A project whose crate's function holds `body`.
fn project(name: &str, body: &str) -> std::path::PathBuf {
    custom_project(
        name,
        &[
            ("Cargo.toml", manifest("").as_str()),
            ("src/lib.rs", crate_with(body).as_str()),
        ],
        PROGRAM,
    )
}

/// Whether the toolchain can check a crate; a test without one ends here.
fn toolchain_checks_crates() -> bool {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    toolchain.cargo.is_some() && toolchain.sdk.is_some()
}

/// The crate's diagnostics of a check that failed on them.
fn crate_errors(error: &anyhow::Error) -> Vec<RustDiagnostic> {
    error
        .chain()
        .find_map(|cause| match cause.downcast_ref::<NativeError>() {
            Some(NativeError::Rust(failure)) => Some(failure.diagnostics.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a crate failure: {error:#}"))
}

#[test]
fn a_project_with_rust_pous_checks_with_its_crate() {
    // Catches the check that generates without the crate: it refused QUICK as a Rust POU "without
    // its Rust item" and failed the project, so VS Code's Compile failed on every Rust project.
    let root = project("rust-check-clean", "");
    if !toolchain_checks_crates() {
        return;
    }
    let report = check_program(&root, &[]).unwrap_or_else(|error| panic!("{error:#}"));
    assert_eq!(report.crate_warnings, Vec::<RustDiagnostic>::new());
    assert!(!root.join("program.tpi").exists(), "the check writes no image");
}

#[test]
fn a_crate_error_fails_the_check_where_it_is() {
    // A text where the function takes a number is the compiler's E0308 on line 10 of the crate's
    // lib.rs, the line of `let wrong`, named as the project names the file. Catches a check that
    // does not compile the crate, and one that reports the error without its place.
    let root = project("rust-check-error", "    let wrong: i32 = \"text\";\n    let _ = wrong;\n");
    if !toolchain_checks_crates() {
        return;
    }
    let error = check_program(&root, &[]).expect_err("the crate does not compile");
    let errors: Vec<_> = crate_errors(&error)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == RustSeverity::Error)
        .map(|diagnostic| (diagnostic.code, diagnostic.file, diagnostic.line))
        .collect();
    assert_eq!(
        errors,
        [(Some("E0308".to_string()), "rust/src/lib.rs".to_string(), 10)]
    );
}

#[test]
fn a_crate_warning_leaves_the_check_passed() {
    // An unused variable on line 10 is a warning: the check passes and names it where it is.
    // Catches a warning dropped, and one that fails the check.
    let root = project("rust-check-warning", "    let unused = 1;\n");
    if !toolchain_checks_crates() {
        return;
    }
    let report = check_program(&root, &[]).unwrap_or_else(|error| panic!("{error:#}"));
    let warnings: Vec<_> = report
        .crate_warnings
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.severity,
                diagnostic.file.as_str(),
                diagnostic.line,
                diagnostic.message.contains("unused variable"),
            )
        })
        .collect();
    assert_eq!(
        warnings,
        [(RustSeverity::Warning, "rust/src/lib.rs", 10, true)]
    );
}

#[test]
fn a_crate_without_a_toolchain_is_refused_with_its_reason() {
    // Without a toolchain the crate is not checked, and the check says so instead of passing a
    // project whose Rust it never compiled. The crate is read without cargo, so the test needs
    // no toolchain.
    let root = project("rust-check-no-toolchain", "");
    let target = Target::this_cpu().expect("a computer target");
    let refused = check_project(
        &root,
        &sources(&root),
        &Options {
            target,
            hooks: true,
        },
        ConversionProfile::Iec,
        || Err(NativeError::Toolchain("no toolchain here".into())),
        &root.join("build/native"),
    )
    .expect_err("no toolchain");
    let NativeError::Toolchain(message) = refused else {
        panic!("a toolchain failure: {refused}");
    };
    assert!(
        message.contains("checked with the toolchain's cargo") && message.contains("no toolchain here"),
        "{message}"
    );
}

/// `trust-runtime check` of the project in `root` with `args`.
fn check_command(root: &Path, args: &[&str]) -> std::process::Output {
    let binary = std::env::var_os("CARGO_BIN_EXE_trust-runtime")
        .expect("Cargo provides trust-runtime while it runs integration tests");
    Command::new(binary)
        .arg("check")
        .args(args)
        .arg("--project")
        .arg(root)
        .output()
        .expect("trust-runtime check runs")
}

#[test]
fn the_command_reports_the_crates_warning_as_an_issue() {
    // The JSON response: passed, one warning, the issue with the compiler as its source and the
    // crate's file; human output: the warning after the source list. Catches a warning that never
    // reaches the response, and human output that hides it.
    let root = project("rust-check-command", "    let unused = 1;\n");
    if !toolchain_checks_crates() {
        return;
    }
    let output = check_command(&root, &["--json"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(response["ok"], serde_json::json!(true), "{response}");
    assert_eq!(response["warnings"], serde_json::json!(1), "{response}");
    let issue = &response["issues"][0];
    assert_eq!(issue["severity"], "warning", "{response}");
    assert_eq!(issue["source"], "rustc", "{response}");
    assert_eq!(issue["file"], "rust/src/lib.rs", "{response}");
    assert_eq!(issue["line"], 10, "{response}");
    let human = check_command(&root, &[]);
    let text = String::from_utf8_lossy(&human.stdout);
    assert!(human.status.success(), "{text}");
    assert!(
        text.contains("Warnings: 1") && text.contains("rust/src/lib.rs:10:"),
        "{text}"
    );
}
