//! The reference integration of Control Studio (checklist V4.7; specification 35, Part D5) on `examples/room_heating`: the real `Control.PID`
//! TIC101 holds the tank at 45 °C, with its loop in Auto. The journey runs through the command line of `trust-control-studio` against a
//! runtime that the test starts outside the engine in simulation mode and attaches to over its control socket.
//!
//! The journey, in the order of the `TypeScript` journey it replaces (`control-studio-room-heating-acceptance.integration.test.ts`):
//! - the heating sequence is started as the operator's HMI Start does: the web HMI logs in as `operator` and invokes the sequence's `start`
//!   on the heating panel (`/api/hmi/command`). The control endpoint has no method invocation, so the test uses the HMI's own route, not a
//!   symbol write;
//! - the test waits until the tank settles at 45 °C with TIC101 in Auto, within two hours of wall time;
//! - `identify` (Automatic: TIC101 stays in Auto), `tune` (Automatic) and `validate`, whose isolated runs carry the qualified model as their
//!   plant (B5.3), and which apply no gain to the running controller;
//! - `apply` with observation: the library state transfer applies the declared states and the Auto entry is confirmed at the output image;
//! - `restore` (the record from before Apply, TIC101 back in Auto), `validate` again when Apply is not offered, `apply` again;
//! - `keep` in project: the running values reach `plant.toml` and the generated declaration of `Main.TIC101_Parameters`.
//!
//! Every check that does not block the next step is recorded and the journey goes on. A blocking step ends the journey, and the test fails
//! with every finding so far. The evidence (the project, the controller, the runtime build from `trust-runtime --version`, the plant, the
//! timing at ten times real time, and each operation with its status and duration) is written to the folder named by the environment
//! variable `TRUST_CONTROL_STUDIO_EVIDENCE`, when it is set, whether or not the journey succeeds.
//!
//! Run time: ten times real time. The room heating's slowest mode, the tank and the room heating together, has a time constant of about
//! four minutes, so the journey takes well under an hour of wall time; the limits below only end a hung run. The test is ignored by
//! default. Run it with `cargo test -p trust-control-studio --test control_studio_d5_acceptance -- --ignored`. It needs the
//! `trust-runtime` and `trust-lsp` executables of the target folder (`cargo build -p trust-runtime --bin trust-runtime -p trust-lsp`).
#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_control::loop_definition::LoopDefinition;
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::loop_inspection_bindings;
use trust_control_studio::loops;
use trust_control_studio::runtime_client::RuntimeClient;

use common::engine::{
    is_auto, read_text, same_values, study_of, value_of, verb_arguments, Findings, Program,
    RUNTIME_ID,
};
use common::{
    copy_dir, inspect_until_available, run_trust_runtime, trust_runtime, wait_for_status, RemoveDir,
};

/// The reference project, a copy of which the journey runs on.
const ROOM_HEATING: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/room_heating");
/// The loop the journey controls: TIC101 of `control-loops/tank-temperature.json`.
const LOOP_FILE: &str = "control-loops/tank-temperature.json";
/// The speed of the simulation, ten times real time.
const TIME_SCALE: u32 = 10;
/// The setpoint at which TIC101 holds the tank, in degrees Celsius (`examples/room_heating/src/Main.st`).
const DESIGN_SETPOINT_C: f64 = 45.0;
/// The tank has settled when TIC101 is in Auto and the tank is within this band of the setpoint (specification 35, D5).
const SETTLED_BAND_C: f64 = 0.5;
/// How long the tank may take to settle.
const HEATING_TIMEOUT: Duration = Duration::from_hours(2);
/// How often the tank is read while it heats.
const HEATING_POLL: Duration = Duration::from_secs(10);
/// The longest that identify, validate or an Apply observation may take: at ten times real time, about an hour at most.
const STEP_LIMIT: Duration = Duration::from_mins(65);
/// The longest that tune may take.
const TUNE_LIMIT: Duration = Duration::from_mins(30);
/// The longest that a review, a restore or a keep may take.
const SHORT_LIMIT: Duration = Duration::from_mins(5);
/// How often the journey presses Validate while the engine answers that the tank is still changing (B3.4 step 2).
const VALIDATE_ATTEMPTS: u32 = 3;
/// The faceplate of the heating sequence on the operator's HMI; its `start` object starts the sequence.
const HEATING_PANEL: &str = "overview/heating-panel";
/// The symbol that TIC101's state transfer writes first: its presence marks the library state transfer.
const NOMINAL_OUTPUT: &str = "Main.TIC101.StateValue.NominalOutput";
/// The instance of TIC101, whose record `plant.toml` keeps as `[equipment."Main.TIC101".settings]`.
const INSTANCE: &str = "Main.TIC101";

/// The operations of the journey and the checks that failed, written as evidence.
#[derive(Default)]
struct Record {
    operations: Vec<Value>,
    findings: Findings,
}

impl Record {
    /// Records an operation that started at `started` and ended with `status`, with its duration.
    fn operation(&mut self, name: &str, started: Instant, status: &str) {
        let duration = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.operations
            .push(json!({ "operation": name, "status": status, "durationMs": duration }));
    }
}

/// The copy of the project: its folders and the loop that the verbs name.
struct ProjectCopy {
    /// The folder of the test: the project copy, the journal folder and the output files of the verbs.
    folder: PathBuf,
    /// The copied project.
    project: PathBuf,
    /// The folder of the operation journals.
    journal: PathBuf,
    /// The loop's identifier.
    loop_id: String,
}

impl ProjectCopy {
    /// Runs `verb` on the loop with `extra` to its end. The copy's runtime is a unix socket with no token, so the environment carries none.
    /// The output goes to files named `name`.
    fn run(
        &self,
        name: &str,
        verb: &str,
        extra: &[&str],
        limit: Duration,
    ) -> (Program, ExitStatus) {
        let arguments = verb_arguments(verb, &self.project, &self.loop_id, &self.journal, extra);
        let mut program = Program::start(&arguments, false, &self.folder, name);
        let status = program.wait(limit);
        (program, status)
    }
}

/// The runtime started outside the engine, in simulation mode at ten times real time; dropping it kills the process.
struct AttachedRuntime(Child);

impl AttachedRuntime {
    /// Starts `trust-runtime run --simulation` on the project, with its output written to `log`.
    fn start(project: &Path, log: &Path) -> Self {
        let output = File::create(log).expect("the runtime's log");
        let errors = output.try_clone().expect("share the log");
        let child = Command::new(trust_runtime())
            .args([
                "run",
                "--project",
                &project.display().to_string(),
                "--simulation",
                "--time-scale",
                &TIME_SCALE.to_string(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::from(output))
            .stderr(Stdio::from(errors))
            .spawn()
            .expect("start trust-runtime run");
        Self(child)
    }
}

impl Drop for AttachedRuntime {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Points the copy's control endpoint at a socket in the copy, switches its web server on at `web_port` (the operator's HMI route), and
/// makes every loop of the copy bind the runtime that the verbs name.
fn prepare_attached_project(project: &Path, socket: &Path, web_port: u16) {
    let runtime_path = project.join("runtime.toml");
    let text = set_toml_keys(
        &read_text(&runtime_path),
        "runtime.control",
        &[("endpoint", format!("\"unix://{}\"", socket.display()))],
    );
    let text = set_toml_keys(
        &text,
        "runtime.web",
        &[
            ("enabled", "true".to_string()),
            ("listen", format!("\"127.0.0.1:{web_port}\"")),
        ],
    );
    fs::write(&runtime_path, text).expect("write runtime.toml");
    for entry in fs::read_dir(project.join("control-loops")).expect("the loops folder") {
        let path = entry.expect("an entry").path();
        let mut loop_json = json_of(&path);
        loop_json["runtimeId"] = json!(RUNTIME_ID);
        fs::write(&path, format!("{loop_json:#}\n")).expect("write the loop");
    }
}

/// Sets keys of one table of a TOML text, replacing their lines, and adds the keys that the table lacks.
fn set_toml_keys(text: &str, table: &str, pairs: &[(&str, String)]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for (key, value) in pairs {
        let header = lines
            .iter()
            .position(|line| line.trim() == format!("[{table}]"))
            .unwrap_or_else(|| panic!("runtime.toml has no [{table}] table"));
        let end = lines[header + 1..]
            .iter()
            .position(|line| line.trim_start().starts_with('['))
            .map_or(lines.len(), |offset| header + 1 + offset);
        let prefix = format!("{key} =");
        match (header + 1..end).find(|index| lines[*index].trim_start().starts_with(&prefix)) {
            Some(index) => lines[index] = format!("{key} = {value}"),
            None => lines.insert(header + 1, format!("{key} = {value}")),
        }
    }
    format!("{}\n", lines.join("\n"))
}

/// The JSON of a file, or `Value::Null` when it is missing or is not JSON.
fn json_of(path: &Path) -> Value {
    serde_json::from_str(&read_text(path)).unwrap_or_default()
}

/// The names of the plant in the copy's `simulation.toml` (`name = "…"`), which the evidence names.
fn plant_names(project: &Path) -> Vec<String> {
    read_text(&project.join("simulation.toml"))
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "name").then(|| value.trim().trim_matches('"').to_string())
        })
        .collect()
}

/// A port on the loopback interface, free now, for the web server of the copy's runtime.
fn reserve_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    listener.local_addr().expect("the port").port()
}

/// The answer of a request to the web HMI: its status, its cookies and its JSON body.
struct HttpAnswer {
    status: u16,
    set_cookie: Vec<String>,
    body: Value,
}

/// Sends one HTTP/1.1 request to the web HMI on `port` and reads its answer. The request asks for the connection to close; the answer is
/// read by its `Content-Length` or to the end of its chunks.
fn http(
    port: u16,
    method: &str,
    route: &str,
    cookie: Option<&str>,
    body: Option<&Value>,
) -> Result<HttpAnswer, String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|error| format!("the web HMI does not accept connections: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .map_err(|error| error.to_string())?;
    let payload = body.map(Value::to_string).unwrap_or_default();
    let cookie_line = cookie.map_or_else(String::new, |cookie| format!("Cookie: {cookie}\r\n"));
    let request = format!(
        "{method} {route} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{cookie_line}\r\n{payload}",
        payload.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("the request cannot be sent: {error}"))?;
    let mut raw = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| format!("the answer cannot be read: {error}"))?;
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&buffer[..read]);
        if answer_is_complete(&raw) {
            break;
        }
    }
    parse_answer(&String::from_utf8_lossy(&raw))
}

/// Whether the raw answer holds its whole body: by its `Content-Length`, or by its last chunk.
fn answer_is_complete(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(raw);
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    let head = head.to_ascii_lowercase();
    if head.contains("transfer-encoding: chunked") {
        return body.ends_with("0\r\n\r\n");
    }
    head.lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|length| length.trim().parse::<usize>().ok())
        .is_some_and(|length| body.len() >= length)
}

/// Reads a whole HTTP answer: its status, its `Set-Cookie` headers and its JSON body.
fn parse_answer(text: &str) -> Result<HttpAnswer, String> {
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "the answer has no header end".to_string())?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| "the answer has no status".to_string())?;
    let set_cookie = head
        .lines()
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("set-cookie")
                .then(|| value.trim().to_string())
        })
        .collect();
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        dechunk(body)
    } else {
        body.to_string()
    };
    Ok(HttpAnswer {
        status,
        set_cookie,
        body: serde_json::from_str(&body).unwrap_or(Value::Null),
    })
}

/// The payload of a chunked body.
fn dechunk(body: &str) -> String {
    let mut payload = String::new();
    let mut rest = body;
    while let Some((size, after)) = rest.split_once("\r\n") {
        let Ok(length) = usize::from_str_radix(size.trim(), 16) else {
            break;
        };
        if length == 0 || after.len() < length {
            break;
        }
        payload.push_str(&after[..length]);
        rest = after[length..]
            .strip_prefix("\r\n")
            .unwrap_or(&after[length..]);
    }
    payload
}

/// Calls `probe` until it gives a value, for `limit` at most.
fn wait_for<T>(
    what: &str,
    limit: Duration,
    mut probe: impl FnMut() -> Option<T>,
) -> Result<T, String> {
    let begun = Instant::now();
    loop {
        if let Some(value) = probe() {
            return Ok(value);
        }
        if begun.elapsed() >= limit {
            return Err(format!(
                "{what} did not appear within {} s",
                limit.as_secs()
            ));
        }
        sleep(Duration::from_millis(500));
    }
}

/// Presses Start on the heating sequence as the operator does: logs in as `operator`, then invokes the panel's `start` through the web
/// HMI's command route. The runtime answers `applied` once the method call is accepted.
fn press_heating_start(port: u16) -> Result<(), String> {
    let bundle = wait_for(
        "the heating panel's HMI bundle",
        Duration::from_secs(60),
        || {
            http(
                port,
                "GET",
                &format!("/api/hmi/popup?faceplate={HEATING_PANEL}"),
                None,
                None,
            )
            .ok()
            .and_then(|answer| answer.body["bundle"].as_str().map(str::to_string))
        },
    )?;
    let login = http(
        port,
        "POST",
        "/api/hmi/login",
        None,
        Some(&json!({ "user": "operator", "password": "operator" })),
    )?;
    if login.status != 200 {
        return Err(format!(
            "the operator login failed with status {}",
            login.status
        ));
    }
    let cookie = login
        .set_cookie
        .first()
        .and_then(|value| value.split(';').next())
        .unwrap_or_default()
        .to_string();
    if cookie.is_empty() {
        return Err("the operator login gave no session cookie".to_string());
    }
    let command = http(
        port,
        "POST",
        "/api/hmi/command",
        Some(&cookie),
        Some(&json!({
            "id": format!("heating-start-{}", std::process::id()),
            "bundle": bundle,
            "page": format!("faceplate:{HEATING_PANEL}"),
            "object": "start",
            "action": "invoke",
        })),
    )?;
    if command.status != 200 || command.body["outcome"] != "applied" {
        return Err(format!(
            "the heating sequence did not accept Start (status {}): {}",
            command.status, command.body
        ));
    }
    Ok(())
}

/// What TIC101 reads from the runtime: its mode, its setpoint and its measurement.
struct Reading {
    auto: Option<bool>,
    setpoint: Option<f64>,
    process_value: Option<f64>,
}

/// Reads TIC101's mode, setpoint and measurement from the runtime (E3).
fn read_loop(
    connection: &RuntimeConnection,
    loop_definition: &LoopDefinition,
    revision: &str,
) -> Reading {
    let bindings = loop_inspection_bindings(loop_definition);
    let mut client = RuntimeClient::connect(connection).expect("the runtime answers");
    let sample = inspect_until_available(&mut client, loop_definition, &bindings, revision);
    Reading {
        auto: is_auto(loop_definition, value_of(&sample, "controllerMode")),
        setpoint: value_of(&sample, "setpoint"),
        process_value: value_of(&sample, "processVariable"),
    }
}

/// Whether the engine refused because the measured output is still changing (B3.4 step 2), which waiting resolves.
fn still_changing(answer: &Value) -> bool {
    answer["failure"]["cause"] == "settings"
        && answer["failure"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("is still changing by"))
}

/// Waits until TIC101 is in Auto at the design setpoint and the tank is within the settled band (D5).
fn wait_for_heated_tank(
    connection: &RuntimeConnection,
    loop_definition: &LoopDefinition,
    revision: &str,
) -> Result<(), String> {
    let begun = Instant::now();
    loop {
        let reading = read_loop(connection, loop_definition, revision);
        let settled = reading.auto == Some(true)
            && reading
                .setpoint
                .is_some_and(|setpoint| (setpoint - DESIGN_SETPOINT_C).abs() < 1e-6)
            && reading
                .process_value
                .is_some_and(|value| (value - DESIGN_SETPOINT_C).abs() <= SETTLED_BAND_C);
        if settled {
            return Ok(());
        }
        if begun.elapsed() >= HEATING_TIMEOUT {
            return Err(format!(
                "TIC101 did not settle at {DESIGN_SETPOINT_C} °C in Auto within {} s of wall time; its last reading: Auto {:?}, setpoint {:?}, measurement {:?}",
                HEATING_TIMEOUT.as_secs(),
                reading.auto,
                reading.setpoint,
                reading.process_value,
            ));
        }
        sleep(HEATING_POLL);
    }
}

/// The value of `member` in the settings table of `instance` in `plant.toml`, or `None` when the table or the key is missing.
fn setting_in_table(plant: &str, instance: &str, member: &str) -> Option<f64> {
    let header = format!("[equipment.\"{instance}\".settings]");
    let table = plant.split_once(&header)?.1.split("\n[").next()?;
    let key = format!("\"{member}\"");
    table
        .lines()
        .find(|line| line.trim_start().starts_with(&key))?
        .split_once('=')?
        .1
        .trim()
        .trim_matches('"')
        .parse()
        .ok()
}

/// The names in the array `field` of `value`, in order.
fn names_of(value: &Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The values that a review's parameters take (`next`), each by its symbol.
fn next_values(review: &Value) -> BTreeMap<String, f64> {
    review["parameters"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some((item["symbol"].as_str()?.to_string(), item["next"].as_f64()?))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The text of a string field, or an empty text.
fn text_of(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_string()
}

/// The reference integration of Control Studio on `examples/room_heating` (V4.7, D5). Ignored by default: see the module documentation.
///
/// Run time: ×10 (ten times real time), well under an hour of wall time; the limits only end a hung run (module documentation).
#[test]
#[ignore = "a long journey at ten times real time; run with --ignored"]
#[allow(clippy::too_many_lines)] // The project copy, the runtime, the journey and the evidence, in the order they happen.
fn room_heating_tic101_is_identified_tuned_validated_applied_restored_and_kept() {
    let started = Instant::now();
    let folder = std::env::temp_dir().join(format!("trust-cs-d5-{}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    let _remove = RemoveDir(folder.clone());
    let project = folder.join("project");
    copy_dir(Path::new(ROOM_HEATING), &project);
    let socket = project.join(".control.sock");
    assert!(
        socket.as_os_str().len() <= 100,
        "the socket path {} is too long; use a shorter TMPDIR",
        socket.display()
    );
    let web_port = reserve_port();
    prepare_attached_project(&project, &socket, web_port);
    run_trust_runtime(&["build", "--project", &project.display().to_string()]);
    let loop_file = json_of(&project.join(LOOP_FILE));
    let declared = LoopDefinition::decode(&read_text(&project.join(LOOP_FILE)))
        .expect("the copy's loop decodes");
    let loop_definition = loops::open(&project, RUNTIME_ID, &declared.id)
        .expect("Control Studio opens the loop of the copy");
    let runtime_build = Command::new(trust_runtime())
        .arg("--version")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();

    let log = project.join("runtime.log");
    let _runtime = AttachedRuntime::start(&project, &log);
    let connection = RuntimeConnection {
        endpoint: format!("unix://{}", socket.display()),
        token: None,
    };
    let first = wait_for_status(&connection, &log);
    assert_eq!(
        first.simulation_time_scale, TIME_SCALE,
        "the runtime runs at ten times real time"
    );
    let copy = ProjectCopy {
        folder: folder.clone(),
        project: project.clone(),
        journal: folder.join("journal"),
        loop_id: loop_definition.id.clone(),
    };
    let mut record = Record::default();
    let outcome = journey(
        &copy,
        &connection,
        &loop_definition,
        &loop_file,
        &first.program_revision,
        web_port,
        &mut record,
    );
    if let Err(blocked) = &outcome {
        record
            .findings
            .check(false, format!("the journey stopped: {blocked}"));
    }

    if let Ok(evidence_folder) = std::env::var("TRUST_CONTROL_STUDIO_EVIDENCE") {
        let controller = &loop_file["controller"];
        let evidence = json!({
            "journey": "Control Studio room heating acceptance (V4.7, specification 35, Part D5)",
            "project": {
                "fixture": "examples/room_heating",
                "projectId": loop_definition.project_id,
                "loopId": loop_definition.id,
                "loopRevision": loop_definition.revision,
            },
            "controller": {
                "instance": controller["instance"],
                "type": controller["type"],
                "taskPeriodMs": controller["taskPeriodMs"],
                "revision": controller["revision"],
                "modeSignal": controller["modeSignal"],
            },
            "runtime": {
                "build": runtime_build,
                "timeScale": TIME_SCALE,
                "attachedAs": "a runtime started outside the engine in simulation mode, attached over its unix control socket",
            },
            "plant": plant_names(&project),
            "timing": { "wallMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX) },
            "operations": record.operations,
            "problems": record.findings.failed(),
        });
        let evidence_folder = PathBuf::from(evidence_folder);
        fs::create_dir_all(&evidence_folder).expect("the evidence folder");
        fs::write(
            evidence_folder.join("room-heating-acceptance.json"),
            format!("{evidence:#}\n"),
        )
        .expect("write the evidence");
    }
    record.findings.conclude();
}

/// The journey's steps in order. A blocking step returns its message as the error; a check that does not block records its finding in
/// `record` and the journey goes on.
#[allow(clippy::too_many_lines)] // One journey in the engineer's order; each step depends on the one before it.
fn journey(
    copy: &ProjectCopy,
    connection: &RuntimeConnection,
    loop_definition: &LoopDefinition,
    loop_file: &Value,
    revision: &str,
    web_port: u16,
    record: &mut Record,
) -> Result<(), String> {
    let loop_id = copy.loop_id.as_str();
    let client = |connection: &RuntimeConnection| {
        RuntimeClient::connect(connection).expect("the runtime answers")
    };

    // The heating sequence starts as the operator's HMI Start does, and the tank settles at the design setpoint in Auto.
    let heating = Instant::now();
    press_heating_start(web_port)?;
    record.operation(
        "Start the heating sequence (operator, HMI Start)",
        heating,
        "applied",
    );
    wait_for_heated_tank(connection, loop_definition, revision)?;
    record.operation(
        "Tank settles at 45 °C with TIC101 in Auto",
        heating,
        "settled",
    );

    // Identify with Automatic: a closed-loop setpoint step, after which TIC101 must still be in Auto.
    let started = Instant::now();
    let (program, status) = copy.run("identify", "identify", &["--json"], STEP_LIMIT);
    let identified = status.code() == Some(0) && program.answer().get("result").is_some();
    record.operation(
        "Identify (Automatic)",
        started,
        if identified { "model-ready" } else { "failed" },
    );
    if !identified {
        return Err(format!(
            "identify did not finish with its result:\n{}",
            program.report(status)
        ));
    }
    record.findings.check(
        read_loop(connection, loop_definition, revision).auto == Some(true),
        "Identify with Automatic keeps TIC101 in Auto (A10.6, B3.3)",
    );
    record.findings.check(
        study_of(&copy.project, loop_id)["sections"]["identify"].is_object(),
        "Identify keeps its model in the study (B10.1)",
    );

    // Tune with Automatic: a candidate, unapplied. The running gains are read before Tune and compared after Validate.
    let values_before_tune = client(connection)
        .parameter_values(loop_definition)
        .expect("the parameter record");
    let started = Instant::now();
    let (program, status) = copy.run("tune", "tune", &["--json"], TUNE_LIMIT);
    let candidate = program.answer()["result"]["candidate"].clone();
    let tuned = status.code() == Some(0) && candidate.is_object();
    record.operation(
        "Tune (Automatic)",
        started,
        if tuned { "candidate" } else { "failed" },
    );
    if !tuned {
        return Err(format!(
            "tune did not design a candidate:\n{}",
            program.report(status)
        ));
    }
    record.findings.check(
        candidate["applied"] == false,
        "the candidate is unapplied (B4.1)",
    );

    // Identify does not wait for the tank to come back from its setpoint step (B3.4 step 5), and Validate's model check starts from a
    // steady baseline, which it waits for up to the longer of 5 minutes and 20 % of its maximum duration (B3.4 step 2). As an
    // engineer does, the journey waits for the tank first, and presses Validate again while the engine answers that the tank is
    // still changing.
    let settling = Instant::now();
    wait_for_heated_tank(connection, loop_definition, revision)?;
    record.operation("Tank back at 45 °C after Identify", settling, "settled");

    // Validate: the model check, then the isolated runs, which carry the qualified model as their plant (B5.3).
    let started = Instant::now();
    let mut attempt = 1;
    let (program, status) = loop {
        let name = format!("validate-{attempt}");
        let (program, status) = copy.run(&name, "validate", &["--json"], STEP_LIMIT);
        if attempt < VALIDATE_ATTEMPTS && still_changing(&program.answer()) {
            attempt += 1;
            continue;
        }
        break (program, status);
    };
    let validated = program.answer()["result"].clone();
    record.operation(
        "Validate (model check and isolated simulation)",
        started,
        if validated["applyAvailable"] == true {
            "verified"
        } else {
            "not verified"
        },
    );
    if status.code() != Some(0) && validated.is_null() {
        return Err(format!(
            "validate did not finish with its result:\n{}",
            program.report(status)
        ));
    }
    let scenarios = validated["scenarios"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    record.findings.check(
        !scenarios.is_empty()
            && scenarios
                .iter()
                .all(|scenario| text_of(scenario, "plant").starts_with("model ")),
        format!(
            "every isolated run carries the qualified model as its plant (B5.3): {scenarios:?}"
        ),
    );
    record.findings.check(
        same_values(
            &client(connection)
                .parameter_values(loop_definition)
                .expect("the parameter record"),
            &values_before_tune,
        ),
        "tuning and the isolated validation apply no gain to the running controller (N8.9)",
    );
    if validated["applyAvailable"] != true {
        return Err(format!(
            "Apply is not offered for the validated candidate: {validated}"
        ));
    }

    // Apply with observation: the state transfer applies the declared states, and the Auto entry is confirmed at the output image.
    let values_before_apply = client(connection)
        .parameter_values(loop_definition)
        .expect("the parameter record");
    let (program, status) = copy.run("apply-review", "apply", &["--json"], SHORT_LIMIT);
    let review = program.answer()["result"].clone();
    if status.code() != Some(0) || !text_of(&review, "identity").starts_with("sha256:") {
        return Err(format!(
            "apply did not answer its review:\n{}",
            program.report(status)
        ));
    }
    let started = Instant::now();
    let (program, status) = copy.run(
        "apply",
        "apply",
        &["--review", text_of(&review, "identity").as_str(), "--json"],
        STEP_LIMIT,
    );
    let applied = program.answer();
    let applied_ok = status.code() == Some(0) && applied["result"]["result"]["status"] == "passed";
    record.operation(
        "Apply (commissioning with observation)",
        started,
        if applied_ok { "commissioned" } else { "failed" },
    );
    if !applied_ok {
        return Err(format!(
            "the Apply did not pass:\n{}",
            program.report(status)
        ));
    }
    let transfers = applied["result"]["result"]["transfers"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let state_transfer = transfers
        .iter()
        .find(|transfer| transfer["transaction"]["applied_values"][NOMINAL_OUTPUT].is_number());
    record.findings.check(
        state_transfer.is_some(),
        "the library state transfer was observed (A6.4)",
    );
    if let Some(transfer) = state_transfer {
        let transaction = &transfer["transaction"];
        let process_value = loop_file["signals"]["processVariable"]["symbol"]
            .as_str()
            .unwrap_or_default();
        let declared_states = loop_file["controller"]["pidInitialization"]["states"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for state in declared_states {
            let symbol = text_of(&state, "symbol");
            record.findings.check(
                transaction["observed_values"][symbol.as_str()]
                    .as_f64()
                    .is_some_and(f64::is_finite),
                format!("{symbol} is observed after the transfer"),
            );
            if symbol == process_value {
                record.findings.check(
                    transaction["applied_values"][symbol.as_str()].is_null(),
                    format!("{symbol} is the loop's measurement, which the transfer does not write (A6.4)"),
                );
            } else {
                record.findings.check(
                    transaction["applied_values"][symbol.as_str()].is_number(),
                    format!("{symbol} is applied as the declared state"),
                );
            }
        }
        record.findings.check(
            transaction["confirmation"] == "output-image",
            "the Auto entry is confirmed at the output image (E6.7)",
        );
    }
    record.findings.check(
        applied["result"]["restoreAvailable"] == true && applied["result"]["keepAvailable"] == true,
        "after Apply, Restore previous settings and Keep in project are offered (B6.4, B6.5)",
    );
    record.findings.check(
        same_values(
            &next_values(&review),
            &client(connection)
                .parameter_values(loop_definition)
                .expect("the parameter record"),
        ),
        "the runtime's record holds the reviewed candidate (E7.3)",
    );
    record.findings.check(
        read_loop(connection, loop_definition, revision).auto == Some(true),
        "TIC101 stays in Auto after Apply (B6.2)",
    );

    // Restore previous settings: the record from before Apply, and TIC101 back in Auto, the mode it had before Apply (A6.2, B6.4).
    let (program, status) = copy.run("restore-review", "restore", &["--json"], SHORT_LIMIT);
    let restore_review = program.answer()["result"].clone();
    if status.code() != Some(0) {
        return Err(format!(
            "restore did not answer its review:\n{}",
            program.report(status)
        ));
    }
    let started = Instant::now();
    let (program, status) = copy.run(
        "restore",
        "restore",
        &[
            "--review",
            text_of(&restore_review, "identity").as_str(),
            "--json",
        ],
        SHORT_LIMIT,
    );
    let restored = status.code() == Some(0) && program.answer()["result"]["restored"] == true;
    record.operation(
        "Restore previous settings",
        started,
        if restored { "applied" } else { "failed" },
    );
    if !restored {
        return Err(format!(
            "the Restore was not confirmed by the runtime:\n{}",
            program.report(status)
        ));
    }
    record.findings.check(
        same_values(
            &client(connection)
                .parameter_values(loop_definition)
                .expect("the parameter record"),
            &values_before_apply,
        ),
        "Restore returns the parameter values from before Apply (B6.4)",
    );
    record.findings.check(
        read_loop(connection, loop_definition, revision).auto == Some(true),
        "Restore returns TIC101 to Auto, the mode it had before Apply (A6.2, B6.4)",
    );

    // Apply again: when the review refuses, the candidate is validated once more (B7), then the review is made again.
    let (_refused, refusal) = copy.run("apply-review-again", "apply", &["--json"], SHORT_LIMIT);
    if refusal.code() != Some(0) {
        let (validated_again, status) =
            copy.run("validate-again", "validate", &["--json"], STEP_LIMIT);
        if status.code() != Some(0) {
            return Err(format!(
                "validate again did not verify the candidate:\n{}",
                validated_again.report(status)
            ));
        }
    }
    let (program, status) = copy.run("apply-review-third", "apply", &["--json"], SHORT_LIMIT);
    let review_again = program.answer()["result"].clone();
    if status.code() != Some(0) {
        return Err(format!(
            "apply did not answer its review again:\n{}",
            program.report(status)
        ));
    }
    let started = Instant::now();
    let (program, status) = copy.run(
        "apply-again",
        "apply",
        &[
            "--review",
            text_of(&review_again, "identity").as_str(),
            "--json",
        ],
        STEP_LIMIT,
    );
    let applied_again =
        status.code() == Some(0) && program.answer()["result"]["result"]["status"] == "passed";
    record.operation(
        "Apply again",
        started,
        if applied_again {
            "commissioned"
        } else {
            "failed"
        },
    );
    if !applied_again {
        return Err(format!(
            "the second Apply did not pass:\n{}",
            program.report(status)
        ));
    }

    // Keep in project: the running values become the record's values in the project (B6.5), in plant.toml and the declaration.
    let main_before = read_text(&copy.project.join("src").join("Main.st"));
    let (program, status) = copy.run("keep-review", "keep", &["--json"], SHORT_LIMIT);
    let keep_review = program.answer()["result"].clone();
    if status.code() != Some(0)
        || !names_of(&keep_review, "kept")
            .iter()
            .any(|name| name.ends_with("ProportionalGain"))
    {
        return Err(format!(
            "keep did not review the record:\n{}",
            program.report(status)
        ));
    }
    let started = Instant::now();
    let (program, status) = copy.run(
        "keep",
        "keep",
        &[
            "--review",
            text_of(&keep_review, "identity").as_str(),
            "--json",
        ],
        SHORT_LIMIT,
    );
    let kept = program.answer();
    let kept_ok = status.code() == Some(0) && !names_of(&kept["result"], "kept").is_empty();
    record.operation(
        "Keep in project",
        started,
        if kept_ok { "kept" } else { "failed" },
    );
    if !kept_ok {
        return Err(format!(
            "Keep in project was not confirmed:\n{}",
            program.report(status)
        ));
    }
    let plant = read_text(&copy.project.join("plant.toml"));
    let running = client(connection)
        .parameter_values(loop_definition)
        .expect("the parameter record");
    let running_kp = running
        .get(&format!("{INSTANCE}_Parameters.ProportionalGain"))
        .copied();
    record.findings.check(
        setting_in_table(&plant, INSTANCE, "{instance}_Parameters.ProportionalGain")
            .zip(running_kp)
            .is_some_and(|(kept_kp, running_kp)| {
                (kept_kp - running_kp).abs() <= 1e-6 * running_kp.abs().max(1.0)
            }),
        "plant.toml keeps the running proportional gain of TIC101 (B6.5)",
    );
    record.findings.check(
        read_text(&copy.project.join("src").join("Main.st")) != main_before,
        "the declaration of TIC101's record in the project changes with Keep (B6.5)",
    );
    record.findings.check(
        study_of(&copy.project, loop_id)["sections"]["apply"]["keeps"]
            .as_array()
            .is_some_and(|keeps| keeps.len() == 1),
        "the study keeps the Keep's receipt (A8.1)",
    );
    Ok(())
}
