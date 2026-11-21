//! `trust-dev rust bench` on a project with Rust POUs (specification 48 RU-48): it measures each
//! function, function block and callable method on this computer with its cases and records the
//! longest call of each in `trust.toml` `[timing.measured]`.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`, with cargo and the
//! SDK's sources; with a toolchain without them it checks nothing.

use std::path::Path;
use std::process::Command;

use trust_runtime::native::rust_pous::measured::code;
use trust_runtime::native::{triple, Target, Toolchain};

fn trust_dev() -> Command {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-dev")
            .expect("Cargo must provide trust-dev binary while executing integration tests"),
    )
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    std::fs::write(path, text).expect("the file");
}

/// A filter block with a public method and two cases, a function with a case, and a function whose
/// case divides by zero.
const LIB: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Exponential moving average of one input.
#[trust_fb(name = \"FILTER_EMA\")]
#[budget(\"50us\")]
#[bench_case(x = 1.0e6, alpha = 0.001)]
#[bench_case(x = -1.0e6)]
pub struct FilterEma {
    #[input]
    pub x: f64,
    #[input]
    #[init(0.1)]
    pub alpha: f64,
    #[output]
    pub y: f64,
    primed: bool,
}

impl Body for FilterEma {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        if self.primed {
            self.y += self.alpha * (self.x - self.y);
        } else {
            self.y = self.x;
            self.primed = true;
        }
        Ok(())
    }
}

impl FilterEma {
    /// Starts the average again at the next call.
    #[trust_method]
    #[budget(\"50us\")]
    pub fn reset(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        self.primed = false;
        Ok(())
    }
}

/// The larger of two inputs, limited to a maximum.
#[trust_function(name = \"MAX_LIMITED\")]
#[budget(\"50us\")]
#[bench_case(a = 7, b = 9, limit = 8)]
pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {
    Ok(a.max(b).min(limit))
}

/// A quotient, which faults on a zero divisor as Structured Text's does.
#[trust_function(name = \"QUOTIENT\")]
#[budget(\"50us\")]
#[bench_case(a = 7, b = 0)]
pub fn quotient(cx: &mut Cx, a: i32, b: i32) -> Result<i32, Fault> {
    trust_plc::checked::div(cx, a, b)
}
";

#[test]
fn the_bench_records_each_measured_pou_with_the_code_of_the_image_and_names_a_faulted_case() {
    // RU-48: the bench runs each case 50 scans on this computer and records, for its target, the
    // longest call of FILTER_EMA (two cases), its public method reset (its declared initial
    // values) and MAX_LIMITED, each with the code of the files the program is built from and the
    // device `computer`. QUOTIENT's case divides by zero: the command names the case and the
    // fault, records no time for it, and fails. Catches a POU measured by nothing (a longest of 0),
    // a record of another code than a build computes, a faulted POU that gets a time, and a
    // command that passes over a fault.
    let toolchain = Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let root = trust_test_support::unique_temp_path("trust-dev-rust-bench");
    write(
        &root.join("rust/Cargo.toml"),
        &format!(
            "[package]\nname = \"filters\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\ntrust-plc = \"={}\"\n",
            env!("CARGO_PKG_VERSION")
        ),
    );
    write(&root.join("rust/src/lib.rs"), LIB);
    let declarations = trust_dev()
        .args(["rust", "declarations", "--project"])
        .arg(&root)
        .output()
        .expect("run trust-dev rust declarations");
    assert!(
        declarations.status.success(),
        "{}",
        String::from_utf8_lossy(&declarations.stderr)
    );
    let bench = trust_dev()
        .args(["rust", "bench", "--scans", "50", "--project"])
        .arg(&root)
        .output()
        .expect("run trust-dev rust bench");
    let stdout = String::from_utf8_lossy(&bench.stdout);
    let stderr = String::from_utf8_lossy(&bench.stderr);
    assert!(
        !bench.status.success(),
        "a faulted case fails the bench\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("QUOTIENT: the case a = 7, b = 0:") && stdout.contains("no time recorded"),
        "{stdout}\n{stderr}"
    );
    let host = triple(Target::this_cpu().expect("this computer's target"));
    let expected_code = code(&root, host).expect("the code of the project");
    let text = std::fs::read_to_string(root.join("trust.toml")).expect("trust.toml is written");
    let table: toml::Table = toml::from_str(&text).expect("trust.toml parses");
    let measured = table["timing"]["measured"]
        .as_table()
        .expect("[timing.measured]");
    let mut names: Vec<&str> = measured.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["FILTER_EMA", "FILTER_EMA.reset", "MAX_LIMITED"],
        "{text}"
    );
    for (pou, targets) in measured {
        let record = &targets[host];
        assert!(
            record["longest"]
                .as_integer()
                .is_some_and(|longest| longest > 0),
            "{pou} was called: {text}"
        );
        assert_eq!(
            record["code"].as_str(),
            Some(expected_code.as_str()),
            "{pou}: {text}"
        );
        assert_eq!(record["device"].as_str(), Some("computer"), "{pou}: {text}");
    }
}
