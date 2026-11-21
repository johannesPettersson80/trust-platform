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

/// A filter block with a public method and two cases, a function with a case, a function whose
/// case divides by zero, and a program with a case that the configuration runs.
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

/// Counts the scans its start input is set in.
#[trust_program(name = \"CELL\")]
#[budget(\"50us\")]
#[bench_case(start = true)]
pub struct Cell {
    #[input]
    pub start: bool,
    #[output]
    pub count: i32,
}

impl Body for Cell {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        if self.start {
            self.count = trust_plc::checked::add(cx, self.count, 1)?;
        }
        Ok(())
    }
}
";

/// The configuration that runs one instance of CELL.
const CONFIGURATION: &str = "CONFIGURATION Line
RESOURCE Cells ON PLC
TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM Cell1 WITH Fast : CELL;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn the_bench_records_each_measured_pou_with_the_code_of_the_image_and_names_a_faulted_case() {
    // RU-48: the bench runs each case 50 scans on this computer and records, for its target, the
    // longest call of FILTER_EMA (two cases), its public method reset (its declared initial
    // values), MAX_LIMITED and CELL, whose case sets the input of the configuration's instance
    // Cell1 before each scan, each with the code of the files the program is built from and the
    // device `computer`. QUOTIENT's case divides by zero: the command names the case and the
    // fault, records no time for it, and fails. Catches a POU measured by nothing (a longest of
    // 0), a program the bench cannot reach, a record of another code than a build computes, a
    // faulted POU that gets a time, and a command that passes over a fault.
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
    write(&root.join("src/configuration.st"), CONFIGURATION);
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
        ["CELL", "FILTER_EMA", "FILTER_EMA.reset", "MAX_LIMITED"],
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

/// A filter block with two cases and a limiting function, for the bench on a device.
const DEVICE_LIB: &str = "#![cfg_attr(not(test), no_std)]
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
}

impl Body for FilterEma {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        self.y += self.alpha * (self.x - self.y);
        Ok(())
    }
}

/// The larger of two inputs, limited to a maximum.
#[trust_function(name = \"MAX_LIMITED\")]
#[budget(\"50us\")]
pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {
    Ok(a.max(b).min(limit))
}
";

/// The steps a recording device took: each one's time, restart and run.
type Steps = std::sync::Arc<std::sync::Mutex<Vec<(u64, String, String)>>>;

/// A device behind a TCP bridge whose settings set the control token `token`, which refuses a
/// request without it as a chip does (`missing auth token`), takes the bench's install, and answers
/// each `cycle.step` with Rust POU records, FILTER_EMA's longest call 1000 ns a step so far and
/// MAX_LIMITED's 700 ns; it keeps each step's time, restart and run.
fn recording_device(token: &'static str) -> (String, Steps) {
    use std::io::{BufRead, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind the device's bridge");
    let port = format!(
        "tcp://{}",
        listener.local_addr().expect("the bridge's address")
    );
    let steps = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = std::sync::Arc::clone(&steps);
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the link connects");
        let mut replies = stream.try_clone().expect("the bridge's stream");
        let mut state = "stopped";
        for line in std::io::BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            let request: serde_json::Value = serde_json::from_str(&line).expect("a JSON request");
            let params = &request["params"];
            if request["auth"].as_str() != Some(token) {
                let reply = format!(
                    r#"{{"id":{},"ok":false,"error":"missing auth token"}}"#,
                    request["id"]
                );
                if writeln!(replies, "{reply}").is_err() {
                    break;
                }
                continue;
            }
            let result = match request["type"].as_str().unwrap_or_default() {
                "status" => serde_json::json!({
                    "state": state,
                    "version": env!("CARGO_PKG_VERSION"),
                    "runtime_id": "c6-bench-7",
                }),
                "restart" => {
                    state = "running";
                    serde_json::json!({})
                }
                "shutdown" => {
                    state = "stopped";
                    serde_json::json!({})
                }
                "transfer.begin" => serde_json::json!({"transfer": "t1"}),
                "cycle.step" => {
                    let mut steps = recorded.lock().expect("the steps");
                    steps.push((
                        params["time_ns"].as_u64().unwrap_or(u64::MAX),
                        params["restart"].as_str().unwrap_or_default().to_string(),
                        params["run"].as_str().unwrap_or_default().to_string(),
                    ));
                    let longest = 1_000 * steps.len();
                    serde_json::json!({
                        "time_ns": params["time_ns"],
                        "values": {},
                        "direct": {},
                        "errors": [],
                        "rust_pous": [
                            {"name": "FILTER_EMA", "budget_ns": 50_000, "longest_ns": longest, "over_budget": 0},
                            {"name": "MAX_LIMITED", "budget_ns": 50_000, "longest_ns": 700, "over_budget": 0},
                        ],
                    })
                }
                _ => serde_json::json!({}),
            };
            // A device writes `id` first, and the link takes a reply by its leading `{"id"`.
            let reply = format!(r#"{{"id":{},"ok":true,"result":{result}}}"#, request["id"]);
            if writeln!(replies, "{reply}").is_err() {
                break;
            }
        }
    });
    (port, steps)
}

#[test]
fn a_bench_on_a_device_steps_each_case_and_records_what_its_steps_report() {
    // RU-48 and specification 11 §6.8.1: on a device the bench installs its image as `trust-dev
    // test --device` does and runs each case by stepped scans, the first of a case restarting the
    // program cold, the stepped time going on 1 ms a scan across the cases: FILTER_EMA's two cases
    // and MAX_LIMITED's case of its initial values, 3 scans each, are the steps at 0 to 8 ms, cold
    // at 0, 3 and 6 ms, running TRUST_BENCH_1, _2 and _3. The record of each POU is the longest
    // call the steps' replies gave, 9000 ns and 700 ns, for the device's target with its runtime ID.
    // The device's settings set a token, which the bench takes from `TRUST_CTL_TOKEN` (specification
    // 22), as the chip refused a bench without it on 21 November. Catches a case stepped without its
    // restart or at a time before the last step, records taken from another reply than the steps', a
    // measurement recorded as this computer's, and a caller's token from the environment left out.
    let toolchain = Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let root = trust_test_support::unique_temp_path("trust-dev-rust-bench-device");
    write(
        &root.join("rust/Cargo.toml"),
        &format!(
            "[package]\nname = \"filters\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\ntrust-plc = \"={}\"\n",
            env!("CARGO_PKG_VERSION")
        ),
    );
    write(&root.join("rust/src/lib.rs"), DEVICE_LIB);
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
    let (port, steps) = recording_device("bench-token");
    let bench = trust_dev()
        .args([
            "rust",
            "bench",
            "--scans",
            "3",
            "--target",
            "host",
            "--device",
            &port,
            "--project",
        ])
        .arg(&root)
        .env("TRUST_CTL_TOKEN", "bench-token")
        .output()
        .expect("run trust-dev rust bench --device");
    let stdout = String::from_utf8_lossy(&bench.stdout);
    let stderr = String::from_utf8_lossy(&bench.stderr);
    assert!(bench.status.success(), "{stdout}\n{stderr}");
    let ms = 1_000_000;
    let step = |n: u64, restart: &str, run: &str| (n * ms, restart.to_string(), run.to_string());
    assert_eq!(
        *steps.lock().expect("the steps"),
        [
            step(0, "cold", "TRUST_BENCH_1"),
            step(1, "", "TRUST_BENCH_1"),
            step(2, "", "TRUST_BENCH_1"),
            step(3, "cold", "TRUST_BENCH_2"),
            step(4, "", "TRUST_BENCH_2"),
            step(5, "", "TRUST_BENCH_2"),
            step(6, "cold", "TRUST_BENCH_3"),
            step(7, "", "TRUST_BENCH_3"),
            step(8, "", "TRUST_BENCH_3"),
        ]
    );
    let host = triple(Target::this_cpu().expect("this computer's target"));
    let expected_code = code(&root, host).expect("the code of the project");
    let text = std::fs::read_to_string(root.join("trust.toml")).expect("trust.toml is written");
    let table: toml::Table = toml::from_str(&text).expect("trust.toml parses");
    let record = |pou: &str| {
        let record = &table["timing"]["measured"][pou][host];
        (
            record["longest"].as_integer(),
            record["code"].as_str().map(str::to_string),
            record["device"].as_str().map(str::to_string),
        )
    };
    let device = Some("c6-bench-7".to_string());
    assert_eq!(
        record("FILTER_EMA"),
        (Some(9_000), Some(expected_code.clone()), device.clone()),
        "{text}"
    );
    assert_eq!(
        record("MAX_LIMITED"),
        (Some(700), Some(expected_code), device),
        "{text}"
    );
}
