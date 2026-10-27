//! `identify` at the product boundary (specification 11, "Identify"): the command line against a
//! control endpoint, the refusal of a project whose program uses the identify output by every
//! command that loads a project (`run`, `validate`, `check` and `ide serve`), and the restart of a
//! stopped runtime, which reads `[runtime.identify]` again.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime::bundle_builder::build_program_image;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

fn runtime_binary() -> std::ffi::OsString {
    std::env::var_os("CARGO_BIN_EXE_trust-runtime")
        .expect("Cargo must provide trust-runtime binary while executing integration tests")
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "trust-runtime-identify-{prefix}-{}-{nanos}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&path).expect("create identify test directory");
    path
}

/// A control endpoint that answers one request with `response` and hands the request to the test.
fn spawn_control_server(response: Value) -> (String, Receiver<Value>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind control test server");
    let endpoint = format!(
        "tcp://{}",
        listener.local_addr().expect("control test server address")
    );
    let (request_tx, request_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept control client");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set control test read timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone control test stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read control request");
        request_tx
            .send(serde_json::from_str(line.trim_end()).expect("parse control request"))
            .expect("send captured request");
        writeln!(stream, "{}", serde_json::to_string(&response).unwrap())
            .expect("write control response");
    });
    (endpoint, request_rx, server)
}

fn run_identify(args: &[&str]) -> Output {
    Command::new(runtime_binary())
        .arg("identify")
        .args(args)
        .env_remove("TRUST_CTL_TOKEN")
        .output()
        .expect("run trust-runtime identify")
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("the command prints one JSON line")
}

#[test]
fn cli_identify() {
    // Specification 11, "Identify", command line: `identify --endpoint <endpoint> --token <token>
    // --duration-s <n>` sends `identify` with `{"duration_s": n}` and the token, and prints the
    // runtime's result as one JSON line. Catches a request with another type or parameter name,
    // a token that is not sent, and an answer that is not printed.
    let (endpoint, request_rx, server) = spawn_control_server(json!({
        "id": 1,
        "ok": true,
        "result": { "identifying": true, "duration_s": 7 }
    }));

    let output = run_identify(&[
        "--endpoint",
        &endpoint,
        "--token",
        "cli-token",
        "--duration-s",
        "7",
    ]);
    let request = request_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("captured control request");
    server.join().expect("control test server");

    assert!(
        output.status.success(),
        "identify failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        request,
        json!({
            "id": 1,
            "type": "identify",
            "auth": "cli-token",
            "params": { "duration_s": 7 }
        })
    );
    assert_eq!(
        stdout_json(&output),
        json!({ "identifying": true, "duration_s": 7 })
    );
}

#[test]
fn cli_identify_reads_the_endpoint_and_token_of_the_project() {
    // Specification 11, "Identify", command line: `--project <dir>` reads the control endpoint and
    // the token of that project's `runtime.toml`. Catches a command that ignores the project and
    // sends no token.
    let (endpoint, request_rx, server) = spawn_control_server(json!({
        "id": 1,
        "ok": true,
        "result": { "identifying": true, "duration_s": 12 }
    }));
    let project = unique_temp_dir("project-token");
    // The template names no endpoint or token (specification 11 §6.8.1): the project sets both.
    let template = render_runtime_toml(&SmolStr::new("identify-test"), 10);
    assert!(template.contains("[runtime.control]\n"), "{template}");
    let runtime_toml = template.replacen(
        "[runtime.control]\n",
        &format!("[runtime.control]\nendpoint = \"{endpoint}\"\nauth_token = \"project-token\"\n"),
        1,
    );
    std::fs::write(project.join("runtime.toml"), runtime_toml).expect("write runtime.toml");

    let output = run_identify(&[
        "--project",
        project.to_str().expect("utf-8 path"),
        "--duration-s",
        "12",
    ]);
    let _ = std::fs::remove_dir_all(&project);
    // Checked before the request is awaited: a command that never connects fails here instead of
    // leaving the test waiting for a request.
    assert!(
        output.status.success(),
        "identify failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let request = request_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("captured control request");
    server.join().expect("control test server");
    assert_eq!(request["auth"], json!("project-token"));
    assert_eq!(request["params"], json!({ "duration_s": 12 }));
}

#[test]
fn cli_identify_prints_the_refusal_of_the_runtime_and_fails() {
    // Specification 11, "Identify", command line: a refused request is a command failure that
    // prints the runtime's error text and its error code. Catches a command that exits with
    // success when the runtime refused, and one that drops the code.
    let (endpoint, request_rx, server) = spawn_control_server(json!({
        "id": 1,
        "ok": false,
        "error": "identify is not configured: set [runtime.identify] output in runtime.toml",
        "error_code": "identify_not_configured"
    }));

    let output = run_identify(&["--endpoint", &endpoint, "--duration-s", "10"]);
    request_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("captured control request");
    server.join().expect("control test server");

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("identify is not configured: set [runtime.identify] output in runtime.toml"),
        "stderr was:\n{error}"
    );
    assert!(
        error.contains("(identify_not_configured)"),
        "stderr was:\n{error}"
    );
}

#[test]
fn cli_identify_needs_a_duration() {
    // Specification 11, "Identify", command line: `--duration-s` is part of the command. Catches
    // a default that would blink for a time the user did not choose.
    let output = run_identify(&["--endpoint", "tcp://127.0.0.1:1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--duration-s"),
        "stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `runtime.toml` with the control endpoint a test machine can use on every platform.
fn portable_runtime_toml(setting: &str) -> String {
    let base = render_runtime_toml(&SmolStr::new("identify-load"), 100);
    #[cfg(windows)]
    let base = base.replacen(
        "endpoint = \"unix:///tmp/trust-runtime.sock\"",
        "endpoint = \"tcp://127.0.0.1:0\"\nauth_token = \"trust-ci-token\"",
        1,
    );
    format!("{base}\n[runtime.identify]\noutput = \"{setting}\"\n")
}

/// A built project whose program binds `%QX0.0` and `%QX0.7` (one byte of output image), with the
/// identify output `setting` and the `io.toml` text `io_toml` (loopback when none).
fn built_project(prefix: &str, setting: &str, io_toml: Option<&str>) -> PathBuf {
    let project = unique_temp_dir(prefix);
    std::fs::create_dir_all(project.join("src")).expect("create source directory");
    std::fs::write(project.join("runtime.toml"), portable_runtime_toml(setting))
        .expect("write runtime.toml");
    let io = match io_toml {
        Some(text) => text.to_owned(),
        None => render_io_toml(&build_io_config_auto("loopback").expect("loopback io template")),
    };
    std::fs::write(project.join("io.toml"), io).expect("write io.toml");
    std::fs::write(
        project.join("src").join("main.st"),
        r#"
PROGRAM Main
VAR
    lamp AT %QX0.0 : BOOL;
    spare AT %QX0.7 : BOOL;
END_VAR
lamp := TRUE;
spare := FALSE;
END_PROGRAM
"#,
    )
    .expect("write main source");
    let toolchain = trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = trust_runtime::native::Target::this_cpu().expect("a computer target");
    let report = build_program_image(&project, None, target, &toolchain, false, true)
        .expect("build the program image");
    assert!(report.image.is_file(), "the program image should exist");
    project
}

/// Runs `trust-runtime run --project <project>` and waits for it to end. A runtime that was
/// refused ends at once; one that starts would run until it is told to stop, so the test stops
/// it after `limit` and fails.
fn run_refused_at_load(project: &Path, limit: Duration) -> Output {
    let mut child = Command::new(runtime_binary())
        .arg("run")
        .arg("--project")
        .arg(project)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start trust-runtime run");
    let started = Instant::now();
    loop {
        if child.try_wait().expect("poll trust-runtime run").is_some() {
            return child.wait_with_output().expect("collect trust-runtime run");
        }
        if started.elapsed() > limit {
            child.kill().expect("stop the runtime that was not refused");
            let output = child.wait_with_output().expect("collect trust-runtime run");
            panic!(
                "the runtime started although the project was to be refused at load\nstderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The text of the error that ended `output`, and whether anything ran: the project's
/// `.trust-runtime` folder, where a running runtime keeps its identity, was created.
fn refusal_of(project: &Path) -> String {
    let output = run_refused_at_load(project, Duration::from_secs(60));
    assert_eq!(output.status.code(), Some(1), "a refused load exits with 1");
    assert!(
        !project.join(".trust-runtime").exists(),
        "nothing ran before the refusal"
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn program_using_the_identify_output_is_refused_at_load() {
    // Specification 11, "Identify", load: `trust-runtime run` refuses a project whose program binds
    // the identify output, before anything runs, with an error that names the output and the
    // binding. The program binds `%QX0.7`, so `output = "%QX0.7"` is refused with that binding
    // named. Catches a load that starts a runtime that blinks an output the program writes, and
    // an error that does not say which output or why.
    let project = built_project("program-uses-output", "%QX0.7", None);
    let error = refusal_of(&project);
    let _ = std::fs::remove_dir_all(&project);

    assert!(
        error.contains("identify output %QX0.7 is used by the program: it binds %QX0.7"),
        "stderr was:\n{error}"
    );
}

#[test]
fn an_io_toml_mapping_onto_the_identify_gpio_line_is_refused_at_load() {
    // Specification 11, "Identify", load: a project whose `io.toml` maps the identify GPIO line is
    // refused before anything runs, naming the line. The `gpio` driver maps line 17 to an
    // output, and `output = "gpio:17"` asks to blink that line. Catches a project that would
    // have the program and identify drive one pin.
    let io_toml = r#"
[io]
driver = "gpio"
params = { outputs = [ { address = "%QX0.0", line = 17 } ] }
"#;
    let project = built_project("gpio-line-mapped", "gpio:17", Some(io_toml));
    let error = refusal_of(&project);
    let _ = std::fs::remove_dir_all(&project);

    assert!(
        error.contains("identify output gpio:17 is used by the project: io.toml maps GPIO line 17"),
        "stderr was:\n{error}"
    );
}

#[test]
fn an_identify_output_beyond_the_output_image_is_refused_at_load() {
    // Specification 11, "Identify", load: the output must lie in the output image that the
    // program's bindings make (one byte here), so the drivers keep getting an image of that
    // length. `output = "%QX5.0"` is refused with the image named. Catches an identify output
    // that would be handed to drivers as an image longer than they expect.
    let project = built_project("beyond-image", "%QX5.0", None);
    let error = refusal_of(&project);
    let _ = std::fs::remove_dir_all(&project);

    assert!(
        error.contains("identify output %QX5.0 is outside the output process image of 1 byte"),
        "stderr was:\n{error}"
    );
}

/// Runs `trust-runtime` with `args` and waits for it to end. A command that is refused ends at
/// once; one that is not refused would run on, so the test stops it after `limit` and fails.
fn refused_command(args: &[&std::ffi::OsStr], limit: Duration) -> Output {
    let mut child = Command::new(runtime_binary())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start trust-runtime");
    let started = Instant::now();
    loop {
        if child.try_wait().expect("poll trust-runtime").is_some() {
            return child.wait_with_output().expect("collect trust-runtime");
        }
        if started.elapsed() > limit {
            child.kill().expect("stop the command that was not refused");
            let output = child.wait_with_output().expect("collect trust-runtime");
            panic!(
                "the command was not refused\nstderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn os(text: &str) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(text)
}

#[test]
fn validate_refuses_a_program_that_uses_the_identify_output() {
    // Specification 11, "Identify", load: `trust-runtime validate` makes the load check of `run`,
    // so a project it accepts is a project `run` accepts. The program binds `%QX0.7`, so
    // `output = "%QX0.7"` is refused with the output and the binding named, and `%QX0.3`, a free
    // bit of the one-byte image, is accepted. Catches a `validate` that accepts a project which
    // `run` refuses, and one that refuses a good output.
    let refused = built_project("validate-uses-output", "%QX0.7", None);
    let output = refused_command(
        &[os("validate"), os("--project"), refused.as_os_str()],
        Duration::from_secs(60),
    );
    let _ = std::fs::remove_dir_all(&refused);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a refused project exits with 1"
    );
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("identify output %QX0.7 is used by the program: it binds %QX0.7"),
        "stderr was:\n{error}"
    );

    let accepted = built_project("validate-free-output", "%QX0.3", None);
    let output = refused_command(
        &[os("validate"), os("--project"), accepted.as_os_str()],
        Duration::from_secs(60),
    );
    let _ = std::fs::remove_dir_all(&accepted);
    assert!(
        output.status.success(),
        "validate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_refuses_a_program_that_uses_the_identify_output() {
    // Specification 11, "Identify", load: `trust-runtime check` makes the load check of `run` on the
    // program its sources compile to. The same project is refused as an issue `config.identify`
    // that names the output and the binding, and a free bit is accepted. Catches a `check` that
    // passes a project which `run` refuses.
    let refused = built_project("check-uses-output", "%QX0.7", None);
    let output = refused_command(
        &[
            os("check"),
            os("--project"),
            refused.as_os_str(),
            os("--json"),
        ],
        Duration::from_secs(60),
    );
    let _ = std::fs::remove_dir_all(&refused);
    assert!(!output.status.success(), "check must fail");
    let report: Value = serde_json::from_slice(&output.stdout).expect("check prints JSON");
    assert_eq!(report["ok"], json!(false));
    let issue = report["issues"]
        .as_array()
        .and_then(|issues| {
            issues
                .iter()
                .find(|issue| issue["code"] == "config.identify")
        })
        .unwrap_or_else(|| panic!("an identify issue in {report}"));
    assert_eq!(
        issue["message"],
        json!("identify output %QX0.7 is used by the program: it binds %QX0.7")
    );

    let accepted = built_project("check-free-output", "%QX0.3", None);
    let output = refused_command(
        &[
            os("check"),
            os("--project"),
            accepted.as_os_str(),
            os("--json"),
        ],
        Duration::from_secs(60),
    );
    let _ = std::fs::remove_dir_all(&accepted);
    assert!(
        output.status.success(),
        "check failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn ide_serve_refuses_a_program_that_uses_the_identify_output() {
    // Specification 11, "Identify", load: `trust-runtime ide serve` makes the load check of `run`
    // on the project's sources, before the server starts. The program binds `%QX0.7`, so
    // `output = "%QX0.7"` ends the command with the error that names the output and the binding.
    // Catches an IDE that serves a project which `run` refuses.
    let project = built_project("ide-uses-output", "%QX0.7", None);
    let output = refused_command(
        &[
            os("ide"),
            os("serve"),
            os("--project"),
            project.as_os_str(),
            os("--listen"),
            os("127.0.0.1:0"),
        ],
        Duration::from_secs(60),
    );
    let _ = std::fs::remove_dir_all(&project);

    assert_eq!(
        output.status.code(),
        Some(1),
        "a refused project exits with 1"
    );
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("identify output %QX0.7 is used by the program: it binds %QX0.7"),
        "stderr was:\n{error}"
    );
}

/// The example program with a bit of the output image that the program leaves alone: `Lamp` at
/// `%QX2.0` makes the image three bytes (`CounterLatched` is `%QW0`), and `%QX2.7` is free.
fn runtime_with_a_free_output_bit() -> runtime_process::Runtime {
    runtime_process::ExampleRuntime::new("memory_marker_counter")
        .edit_source(
            "src/Configuration.st",
            "    P1.CounterLatched AT %QW0 : INT;",
            "    P1.CounterLatched AT %QW0 : INT;\n    P1.Lamp AT %QX2.0 : BOOL;",
        )
        .edit_source(
            "src/Main.st",
            "    CounterLatched : INT;",
            "    CounterLatched : INT;\n    Lamp : BOOL;",
        )
        .start()
}

/// Rewrites the project's `runtime.toml` with `output` as `[runtime.identify] output`, or without
/// the section.
fn set_identify_output(runtime: &runtime_process::Runtime, output: Option<&str>) {
    let path = runtime.project().join("runtime.toml");
    let text = std::fs::read_to_string(&path).expect("read runtime.toml");
    let mut text = match text.find("\n[runtime.identify]") {
        Some(at) => text[..at].to_owned() + "\n",
        None => text,
    };
    if let Some(output) = output {
        text.push_str(&format!("\n[runtime.identify]\noutput = \"{output}\"\n"));
    }
    std::fs::write(&path, text).expect("write runtime.toml");
}

fn identify_request(runtime: &runtime_process::Runtime) -> Value {
    runtime.control_request(json!({"id": 1, "type": "identify", "params": {"duration_s": 5}}))
}

fn status_of(runtime: &runtime_process::Runtime) -> Value {
    let status = runtime.control_request(json!({"id": 2, "type": "status"}));
    assert_eq!(status["ok"], json!(true), "{status}");
    status["result"].clone()
}

/// Waits until `status` reports `state`. The control request is read, not the summary line of
/// `ctl status`, whose fault text has spaces when the resource is faulted.
fn wait_for_state(runtime: &runtime_process::Runtime, state: &str) {
    runtime.wait_until(runtime_process::WAIT, state, || {
        status_of(runtime)["state"] == json!(state)
    });
}

/// Stops the program and waits until the runtime reports it stopped.
fn stop_the_program(runtime: &runtime_process::Runtime) {
    runtime.ctl_ok(&["shutdown"]);
    wait_for_state(runtime, "stopped");
}

/// Asks the stopped (or faulted) runtime to load the program again and waits for `state`.
fn restart_until(runtime: &runtime_process::Runtime, state: &str) {
    runtime.ctl_ok(&["restart", "cold"]);
    wait_for_state(runtime, state);
}

#[test]
fn a_reload_from_stopped_reads_the_identify_output_again() {
    // Specification 11, "Identify", and specification 34, STATE-08: a `restart` that finds the
    // resource Stopped loads the program again, as a launch does, and reads `[runtime.identify]`
    // and makes the identify load check with it. The runtime starts without the setting and
    // answers `identify_not_configured`. After `output = "%QX2.7"` is written and the program is
    // stopped and restarted, `identify` is accepted and `status` reports `identifying`. After the
    // setting is removed and the program is stopped and restarted, the request is refused again and
    // the run that was going has ended. With `output = "%QX2.0"`, which the program binds, the
    // restart leaves the resource Faulted with the load refusal; with a free bit again, the next
    // restart loads the program. Catches a reload that keeps the setting of the first launch, one
    // that skips the load check, and a run that outlives the program it was started on.
    let runtime = runtime_with_a_free_output_bit();
    let refused = identify_request(&runtime);
    assert_eq!(refused["ok"], json!(false), "{refused}");
    assert_eq!(refused["error_code"], json!("identify_not_configured"));

    set_identify_output(&runtime, Some("%QX2.7"));
    stop_the_program(&runtime);
    restart_until(&runtime, "running");
    let accepted = identify_request(&runtime);
    assert_eq!(accepted["ok"], json!(true), "{accepted}");
    assert_eq!(
        accepted["result"],
        json!({"identifying": true, "duration_s": 5})
    );
    assert_eq!(status_of(&runtime)["identifying"], json!(true));

    set_identify_output(&runtime, None);
    stop_the_program(&runtime);
    restart_until(&runtime, "running");
    let refused = identify_request(&runtime);
    assert_eq!(refused["error_code"], json!("identify_not_configured"));
    assert_eq!(status_of(&runtime)["identifying"], json!(false));

    set_identify_output(&runtime, Some("%QX2.0"));
    stop_the_program(&runtime);
    restart_until(&runtime, "faulted");
    let fault = status_of(&runtime)["fault"].clone();
    assert!(
        fault
            .as_str()
            .is_some_and(|text| text
                .contains("identify output %QX2.0 is used by the program: it binds %QX2.0")),
        "the fault was {fault}"
    );

    set_identify_output(&runtime, Some("%QX2.7"));
    restart_until(&runtime, "running");
    let accepted = identify_request(&runtime);
    assert_eq!(accepted["ok"], json!(true), "{accepted}");
}
