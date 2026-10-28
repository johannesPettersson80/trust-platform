//! Control Studio speed equivalence: the same scans at time scale 1 and 10.
//!
//! Specification 46, section 6.1: simulation time advances by exactly one resource cycle for each executed cycle, and speed
//! does not change results (items 1 and 7). Annex E1.4 item 6 states the same rule for the runtime protocol.
//!
//! The water-heater lesson runs at time scale 1 and then at time scale 10. Each run records the process variable of its loop
//! for the same number of executions of its task, one sample per execution (annex E4.2 and E4.4). Samples of the same execution
//! must agree in value and in the process time elapsed since the first execution both runs recorded. Each runtime counts its
//! task's executions from its own start (annex E1.1), so the execution is the common key; the clocks of two runtime processes
//! need not share an origin (annex E1.2). It replaces the extension's journey of the same name
//! (removed on 9 October 2026, checklist `rust-logic-migration.md`, V0.10).
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// The lesson both runs load. Its loop is `P1.Controller`, and its program `P1` runs in `MainTask` (src/Main.st).
const LESSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi"
);
/// The task of the configuration in src/Main.st. The loop definition does not name it.
const TASK: &str = "MainTask";
const PROCESS_VARIABLE: &str = "P1.ProcessValue";
/// Task executions recorded by each run: 30 s of process time at the 100 ms task.
const EXECUTIONS: usize = 300;
/// Each run starts its recording at this scan, so both recordings begin close to the same process time.
const START_SCAN: i64 = 20;
/// Executions that both recordings must hold (the extension's journey required 250 of 300).
const MINIMUM_COMMON_SAMPLES: usize = 250;
/// A known control token for the test runtime. It is not a credential.
const TOKEN: &str = "speed-equivalence-test-token";
const SOCKET_NAME: &str = ".trust-control.sock";
/// The longest unix socket path that the control endpoint accepts in the extension's own tests.
const MAX_SOCKET_PATH_BYTES: usize = 100;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const STATUS_DEADLINE: Duration = Duration::from_secs(30);
const SCAN_DEADLINE: Duration = Duration::from_secs(60);
const RECORDING_DEADLINE: Duration = Duration::from_secs(120);

/// One recorded execution: its sequence number, its process time and the value of the process variable.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sample {
    scan: u64,
    time_ns: i64,
    value: Option<f64>,
}

/// A `trust-runtime run` child. Dropping it kills the process and reaps it, so a failed test leaves no runtime behind.
struct Runtime(Child);

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Removes a temporary folder when the test ends, whether it passed or failed.
struct RemoveDir(PathBuf);

impl Drop for RemoveDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn records_the_same_scans_at_time_scale_1_and_10() {
    let slow = record_at_time_scale(1);
    let fast = record_at_time_scale(10);
    let common = common_samples(&slow, &fast);
    assert!(
        common.len() >= MINIMUM_COMMON_SAMPLES,
        "only {} samples are common to the 1x and 10x recordings; at least {MINIMUM_COMMON_SAMPLES} are required",
        common.len()
    );
    let (first_one, first_ten) = common[0];
    for (one, ten) in &common {
        assert_eq!(
            (one.value, one.time_ns - first_one.time_ns),
            (ten.value, ten.time_ns - first_ten.time_ns),
            "the 1x and 10x recordings differ at execution {}: value and process time since execution {} (ns)",
            one.scan,
            first_one.scan
        );
    }
}

/// Runs a copy of the lesson at one time scale, records its process variable for EXECUTIONS executions of MainTask, and stops
/// the runtime. The copy is removed when the function returns.
fn record_at_time_scale(time_scale: u32) -> Vec<Sample> {
    let project = unique_dir(&format!("speed-{time_scale}x"));
    let _remove_project = RemoveDir(project.clone());
    copy_dir(Path::new(LESSON), &project);
    let socket = control_socket(&project);
    prepare_runtime_toml(&project, &socket);
    build_project(&project);

    let loop_definition: Value = serde_json::from_str(
        &fs::read_to_string(project.join("control-loop.json")).expect("read control-loop.json"),
    )
    .expect("control-loop.json is JSON");
    let revision = loop_definition["revision"]
        .as_str()
        .expect("control-loop.json names its revision")
        .to_string();
    let bindings =
        json!([{ "id": "processVariable", "symbol": PROCESS_VARIABLE, "iec_type": "LREAL" }]);

    let mut runtime = start_runtime(&project, time_scale);
    let status = wait_for_status(&socket, &mut runtime, &project.join("runtime.log"));
    assert_eq!(
        status["simulation_time_scale"],
        json!(time_scale),
        "the runtime does not run at time scale {time_scale}"
    );

    let recording = format!("speed-equivalence-{time_scale}x");
    start_recording(&socket, &recording, &revision, &bindings);
    wait_for_executions(&socket, &recording, &revision);
    let stopped = request(
        &socket,
        "control.recording.stop",
        json!({ "recording_id": recording, "loop_revision": revision }),
    )
    .unwrap_or_else(|error| panic!("stop the {time_scale}x recording: {error}"));
    request(
        &socket,
        "control.recording.release",
        json!({ "recording_id": recording, "loop_revision": revision }),
    )
    .unwrap_or_else(|error| panic!("release the {time_scale}x recording: {error}"));

    assert_eq!(
        stopped["acquisition"]["completeness"],
        json!("complete"),
        "the {time_scale}x recording is incomplete: {}",
        stopped["acquisition"]["reason"]
    );
    let samples = decode_samples(&stopped);
    assert_eq!(
        samples.len(),
        EXECUTIONS,
        "the {time_scale}x recording holds {} executions, not {EXECUTIONS}",
        samples.len()
    );
    samples
}

/// The samples that both runs hold of the same execution of the task, paired, in execution order.
fn common_samples(slow: &[Sample], fast: &[Sample]) -> Vec<(Sample, Sample)> {
    let fast_by_execution: BTreeMap<u64, Sample> =
        fast.iter().map(|sample| (sample.scan, *sample)).collect();
    slow.iter()
        .filter_map(|sample| {
            fast_by_execution
                .get(&sample.scan)
                .map(|other| (*sample, *other))
        })
        .collect()
}

/// Decodes the samples of a stop result (annex E4.7): the sequence number, the process time and the first binding's value.
fn decode_samples(stopped: &Value) -> Vec<Sample> {
    stopped["samples"]
        .as_array()
        .expect("the stop result lists its samples")
        .iter()
        .map(|sample| Sample {
            scan: sample["scan"]
                .as_u64()
                .expect("a sample names its sequence number"),
            time_ns: sample["runtime_time_ns"]
                .as_i64()
                .expect("a sample carries its runtime time (annex E1.2)"),
            value: sample["values"][0].as_f64(),
        })
        .collect()
}

/// Starts the recording on a current coherent inspection of its bindings (annex E4.2). A start that races a scan is refused with
/// `recording_unavailable`, and the start is then repeated on the next inspection.
fn start_recording(socket: &Path, recording: &str, revision: &str, bindings: &Value) {
    let deadline = Instant::now() + SCAN_DEADLINE;
    loop {
        let program_revision = wait_for_start_scan(socket, revision, bindings);
        let started = request(
            socket,
            "control.recording.start",
            json!({
                "recording_id": recording,
                "loop_revision": revision,
                "program_revision": program_revision,
                "bindings": bindings,
                "maximum_samples": EXECUTIONS,
                "maximum_duration_ms": 60_000,
                "task": TASK,
                "record_every": 1,
            }),
        );
        match started {
            Ok(_) => return,
            Err(error)
                if error.contains("current coherent inspection") && Instant::now() < deadline =>
            {
                sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("start the recording {recording}: {error}"),
        }
    }
}

/// Inspects the process variable until a coherent scan at or after START_SCAN, and returns the program revision (annex E3).
fn wait_for_start_scan(socket: &Path, revision: &str, bindings: &Value) -> String {
    let deadline = Instant::now() + SCAN_DEADLINE;
    loop {
        let sample = request(
            socket,
            "control.signals.inspect",
            json!({ "loop_revision": revision, "bindings": bindings }),
        )
        .unwrap_or_else(|error| panic!("inspect {PROCESS_VARIABLE}: {error}"));
        if sample["available"] == json!(true)
            && sample["scan"]
                .as_i64()
                .is_some_and(|scan| scan >= START_SCAN)
        {
            return sample["program_revision"]
                .as_str()
                .expect("the inspection names the program revision")
                .to_string();
        }
        assert!(
            Instant::now() < deadline,
            "the runtime did not reach scan {START_SCAN} within {} s",
            SCAN_DEADLINE.as_secs()
        );
        sleep(Duration::from_millis(20));
    }
}

/// Follows the recording until it holds EXECUTIONS samples or has stopped by itself at its sample limit (annex E4.6).
fn wait_for_executions(socket: &Path, recording: &str, revision: &str) {
    let deadline = Instant::now() + RECORDING_DEADLINE;
    loop {
        let page = request(
            socket,
            "control.recording.status",
            json!({ "recording_id": recording, "loop_revision": revision }),
        )
        .unwrap_or_else(|error| panic!("follow the recording {recording}: {error}"));
        let stopped = page["state"] != json!("recording");
        if stopped
            || page["sample_count"]
                .as_u64()
                .is_some_and(|count| count >= EXECUTIONS as u64)
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the recording {recording} held fewer than {EXECUTIONS} executions after {} s",
            RECORDING_DEADLINE.as_secs()
        );
        sleep(Duration::from_millis(50));
    }
}

/// Starts `trust-runtime run --simulation --time-scale N` on the project, with its output in runtime.log.
fn start_runtime(project: &Path, time_scale: u32) -> Runtime {
    let log = fs::File::create(project.join("runtime.log")).expect("create runtime.log");
    let stderr = log.try_clone().expect("share runtime.log");
    let child = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("run")
    .arg("--project")
    .arg(project)
    .arg("--simulation")
    .arg("--time-scale")
    .arg(time_scale.to_string())
    .stdin(Stdio::null())
    .stdout(Stdio::from(log))
    .stderr(Stdio::from(stderr))
    .spawn()
    .expect("spawn trust-runtime run");
    Runtime(child)
}

/// Waits until the runtime answers `status` (annex E1.3), and returns its answer. A runtime that exits first fails the test.
fn wait_for_status(socket: &Path, runtime: &mut Runtime, log: &Path) -> Value {
    let deadline = Instant::now() + STATUS_DEADLINE;
    loop {
        if let Ok(Some(exit)) = runtime.0.try_wait() {
            panic!(
                "trust-runtime exited before it answered status ({exit}); its log:\n{}",
                log_tail(log)
            );
        }
        if let Ok(status) = request(socket, "status", json!({})) {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "trust-runtime did not answer status within {} s; its log:\n{}",
            STATUS_DEADLINE.as_secs(),
            log_tail(log)
        );
        sleep(Duration::from_millis(200));
    }
}

/// The last lines of a runtime log, for a failure message.
fn log_tail(log: &Path) -> String {
    let text = fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(40)..].join("\n")
}

/// Builds the project with `trust-runtime build --project`, as the extension does before it starts a runtime.
fn build_project(project: &Path) {
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("build")
    .arg("--project")
    .arg(project)
    .output()
    .expect("run trust-runtime build");
    assert!(
        output.status.success(),
        "trust-runtime build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Points the copy's control endpoint at a unix socket in the copy, keeps the test token and turns the web server off, so the two
/// runs never share an endpoint or a port.
fn prepare_runtime_toml(project: &Path, socket: &Path) {
    let path = project.join("runtime.toml");
    let text = fs::read_to_string(&path).expect("read runtime.toml");
    let text = set_control_endpoint(&text, socket);
    fs::write(&path, format!("{text}\n[runtime.web]\nenabled = false\n"))
        .expect("write runtime.toml");
}

/// Gives a copy's `runtime.toml` text the run's control endpoint, the socket in the copy, and its known token. Both keys
/// are optional and the lesson names neither (specification 11, section 6.8.0a), so they are set in the
/// `[runtime.control]` table, added there when absent.
fn set_control_endpoint(text: &str, socket: &Path) -> String {
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .expect("parse runtime.toml");
    let control = &mut document["runtime"]["control"];
    control["endpoint"] = toml_edit::value(format!("unix://{}", socket.display()));
    control["auth_token"] = toml_edit::value(TOKEN);
    document.to_string()
}

/// Copies a folder tree. Lesson folders hold only plain files and folders.
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create the copy of the lesson");
    for entry in fs::read_dir(from).expect("read the lesson folder") {
        let entry = entry.expect("read a lesson entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type of a lesson entry").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a lesson file");
        }
    }
}

/// The runtime's control socket: in the project folder while that path fits a socket address
/// (`MAX_SOCKET_PATH_BYTES`), else beside the folder under the folder's name, as under the release
/// guard's long `TMPDIR`.
fn control_socket(project: &Path) -> PathBuf {
    let socket = project.join(SOCKET_NAME);
    if socket.as_os_str().len() <= MAX_SOCKET_PATH_BYTES {
        return socket;
    }
    let name = project
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let short = project.with_file_name(format!("{name}.sock"));
    assert!(
        short.as_os_str().len() <= MAX_SOCKET_PATH_BYTES,
        "the control socket path is {} bytes; use a shorter TMPDIR (the limit is {MAX_SOCKET_PATH_BYTES})",
        short.as_os_str().len()
    );
    short
}

/// A fresh folder under the temporary directory. The name stays short because a unix socket path has a length limit.
fn unique_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("trust-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create the speed equivalence folder");
    path
}

/// One control request on its own connection: a JSON line out and a JSON line back (annex E0.1). A refusal is an `Err` that
/// carries the runtime's error text and error code.
fn request(socket: &Path, operation: &str, params: Value) -> Result<Value, String> {
    let mut stream = UnixStream::connect(socket)
        .map_err(|error| format!("connect {}: {error}", socket.display()))?;
    stream
        .set_read_timeout(Some(REQUEST_TIMEOUT))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(REQUEST_TIMEOUT))
        .map_err(|error| error.to_string())?;
    let line = json!({ "id": 1, "type": operation, "auth": TOKEN, "params": params });
    writeln!(stream, "{line}").map_err(|error| error.to_string())?;
    let mut answer = String::new();
    BufReader::new(stream)
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    let response: Value = serde_json::from_str(&answer)
        .map_err(|error| format!("{operation}: the answer is not JSON: {error}"))?;
    if response["ok"] == json!(true) {
        Ok(response["result"].clone())
    } else {
        Err(format!(
            "{operation} refused: {} ({})",
            response["error"].as_str().unwrap_or("no error text"),
            response["error_code"].as_str().unwrap_or("no error code")
        ))
    }
}
