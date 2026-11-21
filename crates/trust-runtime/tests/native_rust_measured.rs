//! A `production` build needs a current measurement of each Rust POU its tasks run (specification
//! 48 RU-48): it refuses the image without one, or with one of an older image, naming the POUs and
//! the command that measures them, and RU-47's admission sums the larger of each budget and its
//! measured time; the images that `trust-dev test` and `trust-dev rust bench` build, which hold test
//! programs, need none. Every expected value is worked out by hand from RU-46 to RU-48.
//!
//! The tests build Rust crates with the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use std::path::Path;

use rust_pous_support::{custom_project, manifest, project_sources};
use trust_runtime::native::rust_pous::measured::{code, write, Measured};
use trust_runtime::native::{build_project, triple, Options, Target, Toolchain};
use trust_runtime::stdlib::conversions::ConversionProfile;

/// A block whose budget is 2 ms, a function whose budget is 1 µs, and a function no task calls.
const CRATE: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"SLOW\")]
#[budget(\"2ms\")]
pub struct Slow;

impl Body for Slow {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

#[trust_function(name = \"QUICK\")]
#[budget(\"1us\")]
pub fn quick(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {
    Ok(a.wrapping_add(1))
}

#[trust_function(name = \"UNUSED\")]
#[budget(\"1us\")]
pub fn unused(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {
    Ok(a)
}
";

/// A 10 ms task that calls SLOW twice and QUICK once a scan: 2 × 2 ms + 1 µs of declared Rust work.
const PROGRAM: &str = "PROGRAM Line
VAR
    slow : SLOW;
    i : INT;
    n : DINT;
END_VAR
FOR i := 1 TO 2 DO
    slow();
END_FOR;
n := QUICK(a := n);
END_PROGRAM

CONFIGURATION Cell
RESOURCE Cell ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// A project of [`CRATE`] and [`PROGRAM`] in a `production` profile.
fn production_project(name: &str) -> std::path::PathBuf {
    let root = custom_project(
        name,
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", CRATE)],
        PROGRAM,
    );
    std::fs::write(
        root.join("trust.toml"),
        "[timing]\nprofile = \"production\"\n",
    )
    .expect("trust.toml");
    root
}

/// This computer's target triple, which a build here is for.
fn host() -> &'static str {
    triple(Target::this_cpu().expect("a computer target"))
}

/// Records `longest` of each POU of `measured` for this computer, with `code`.
fn measure(root: &Path, measured: &[(&str, u64)], code: &str) {
    let records: Vec<Measured> = measured
        .iter()
        .map(|(pou, longest_ns)| Measured {
            pou: (*pou).to_string(),
            longest_ns: *longest_ns,
            code: code.to_string(),
            device: "computer".to_string(),
        })
        .collect();
    write(root, host(), &records).expect("the measurements are written");
}

/// The result of building the project in `root` for this computer with the test programs `tests`.
fn build(root: &Path, tests: &[smol_str::SmolStr]) -> Result<(), String> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    build_project(
        root,
        &project_sources(root),
        tests,
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        ConversionProfile::Iec,
        &toolchain,
        &root.join("build/native"),
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[test]
fn a_production_build_names_each_rust_pou_its_tasks_run_without_a_measurement() {
    // RU-48: the task runs SLOW and QUICK, so a production image needs a measurement of both for
    // this computer, and the build names them and `trust-dev rust bench`; UNUSED, which no task
    // calls, needs none. Catches a production build that admits declared budgets alone, and one
    // that asks for a measurement of a POU no scan reaches.
    let root = production_project("rust-measured-none");
    let error = build(&root, &[]).expect_err("the image is refused");
    assert!(
        error.contains("a production build needs a current measurement")
            && error.contains(&format!("for {}", host()))
            && error.contains("): QUICK, SLOW;")
            && error.contains("`trust-dev rust bench`"),
        "{error}"
    );
    assert!(!error.contains("UNUSED"), "{error}");
}

#[test]
fn a_measurement_of_an_older_image_is_refused_and_a_current_one_admits_the_build() {
    // RU-48: a measurement counts only with the code of the image built now; one of another code
    // is older than the image. QUICK 300 ns and SLOW 1 ms with the current code are within their
    // budgets, and 2 × 2 ms + 1 µs fits the 10 ms task, so the image builds. Catches a stale
    // measurement taken as current and a current one refused.
    let root = production_project("rust-measured-current");
    measure(&root, &[("SLOW", 1_000_000), ("QUICK", 300)], "older");
    let error = build(&root, &[]).expect_err("an older measurement is refused");
    assert!(
        error.contains("QUICK (measured on an older image), SLOW (measured on an older image)"),
        "{error}"
    );
    let current = code(&root, host()).expect("the image's code");
    measure(&root, &[("SLOW", 1_000_000), ("QUICK", 300)], &current);
    build(&root, &[]).expect("the measured image builds");
}

#[test]
fn the_admission_sums_the_larger_of_each_budget_and_its_measured_time() {
    // RU-48 with RU-47: SLOW measured at 6 ms, above its 2 ms budget, makes the task's Rust work 2 ×
    // 6 ms + 1 µs, more than its 10 ms interval, which a production build refuses; the declared
    // 2 × 2 ms alone fits. Catches a sum of the budgets only, which lets a measured overrun through.
    let root = production_project("rust-measured-sum");
    let current = code(&root, host()).expect("the image's code");
    measure(&root, &[("SLOW", 6_000_000), ("QUICK", 300)], &current);
    let error = build(&root, &[]).expect_err("the measured work overruns the task");
    assert!(
        error.contains("the task Fast may spend") && error.contains("SLOW 2 × 6 ms"),
        "{error}"
    );
}

#[test]
fn an_image_with_test_programs_needs_no_measurement() {
    // RU-48: the images that `trust-dev test` and `trust-dev rust bench` build hold test programs
    // and are never deployed; they need no measurement, or no bench could measure a production
    // project. Catches a test build refused for the measurements it is about to take.
    let root = production_project("rust-measured-tests");
    std::fs::write(
        root.join("src/probe.st"),
        "TEST_PROGRAM Probe\nVAR\n    n : DINT;\nEND_VAR\nn := QUICK(a := n);\nEND_TEST_PROGRAM\n",
    )
    .expect("the test program");
    build(&root, &["Probe".into()]).expect("the test image builds without a measurement");
}
