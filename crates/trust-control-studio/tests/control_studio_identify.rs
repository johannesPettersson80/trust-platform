//! Control Studio's `identify` and `cancel` verbs against a real runtime, through the built program
//! `trust-control-studio` (checklist V1.6 and V1.10c; specification 35, §0.1b, B3.3 to B3.6, B10.3 and A8.4 to A8.6;
//! runtime protocol annex E5 and E11.4 to E11.6).
//!
//! The program runs on a copy of a heater project against `trust-runtime run --simulation` at ten times real time
//! (`time_scale = 10` in the copy's `simulation.toml`, the speed the user chose for lessons). The runtime's endpoint comes
//! from the copy's `runtime.toml`, which the test points at a socket in the copy; the control token goes to the program
//! through `TRUST_CONTROL_TOKEN`, as §0.1b says, and every run names its own `--journal-dir`, so nothing is written to the
//! user's state folder.
//!
//! # The examples
//!
//! The tests of this file use the heater of lesson 30, `examples/control/06-pid-tuning-methods/30-model-based-pi-tuning`.
//! Lesson 30 holds 35 degC with a Manual output of 75 %, and Automatic steps that output (B3.3). `EXAMPLE` and `LOOP_ID`
//! are the only places that name it.
//!
//! Lesson 5 (`examples/control/01-feedback-foundations-and-temperature-control/05-water-heater-pi`), the PI heater that
//! checklist V1.6 names, has the same heater with a scenario of its own: its project owns the experiment, and Automatic
//! runs that scenario and fits its recording (C12, B3.5). `project_scripted.rs` identifies it.
//!
//! # The files
//!
//! This file is the crate root of the test and holds the helpers. `cli_verbs.rs` has the verbs that need no runtime, and
//! `project_scripted.rs` the identification of lesson 5; both are modules of `control_studio_identify/`.
//!
//! # The plant, and what Identify must find
//!
//! The plant in `simulation.toml` (`heater-model-based-pi`) is `x' = A x + B u`, `y = C x + 35`, with `A = [[-0.2]]`,
//! `B = [[0.04, -0.02]]` (the heater in %, the extraction in kW), `C = [[1], [1]]`, `D = 0`, an input delay of 1 s on the
//! heater, and an input offset of -75 % (the Manual command of 75 % is the plant input 0).
//! - `A` is 1 x 1, so it has one eigenvalue: the root of `det(sI - A) = s + 0.2 = 0`, that is `lambda = -0.2`. The dominant
//!   (here the only) time constant is `tau = -1 / Re(lambda) = 1 / 0.2 = 5 s`.
//! - The gain from the heater is `K = -C A^-1 B_heater = -(1) (1 / -0.2) (0.04) = 0.04 / 0.2 = 0.2` degC per %. It is
//!   positive, as the direct action of the loop's controller needs (B3.9).
//! - The 1 s delay is a dead time, not a second pole. The model Identify should find is therefore first order with delay,
//!   `G(s) = 0.2 e^(-s) / (5 s + 1)`; lesson 30's own text reports `G(s) = 0.2 / (4.9 s + 1) e^(-1.1 s)` for it.
//!
//! The tests accept a time constant within a factor of three of 5 s (1.67 s to 15 s) and a gain within 25 % of 0.2.
//!
//! # Findings
//!
//! A failed check of a journey is recorded as a finding and the journey goes on, so one run reports every failure that
//! does not depend on another. It stops where the next step needs the result of the last, and fails with all findings.
//!
//! # Running
//!
//! `cargo test -p trust-control-studio --test control_studio_identify`. The tests run the `trust-runtime` executable of
//! their own target folder, so build it first with `cargo build -p trust-runtime --bin trust-runtime`, or name another one
//! in `TRUST_RUNTIME_BIN` (`cargo test --all` builds it). The file is Unix only, as `control_studio_runtime.rs` is: the
//! runtime's endpoint is a Unix socket, and the interruption is the signal `SIGINT`.
#![cfg(unix)]

mod common;
// The verbs that need no runtime, and the identification of lesson 5; both are modules of this crate.
#[path = "control_studio_identify/cli_verbs.rs"]
mod cli_verbs;
#[path = "control_studio_identify/project_scripted.rs"]
mod project_scripted;

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::Value;
use trust_control::identification::reconcile::StatusRead;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::loop_inspection_bindings;
use trust_control::study::is_recording_id;
use trust_control_studio::journal::{Journal, JournalEntry};
use trust_control_studio::runtime_client::RuntimeClient;
use trust_control_studio::{loops, storage};

use common::{
    copy_dir, inspect_until_available, prepare_runtime_toml, run_trust_runtime, start_runtime,
    wait_for_status, RemoveDir, Runtime, TOKEN,
};

/// The heater project the tests copy (see the module documentation for the choice).
const EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning"
);
/// The identifier of the example's loop (`control-loop.json`).
const LOOP_ID: &str = "control-model-based-tuning";
/// The runtime the verbs name for a project's loops; the example's loop is bound to it.
const RUNTIME_ID: &str = "simulator";
/// The speed of the simulation, ten times real time.
const TIME_SCALE: u32 = 10;
/// The command that the heater holds in Manual before and after the step (`ManualOutput := LREAL#75.0` in `src/Main.st`).
const HOLDING_COMMAND: f64 = 75.0;
/// The plant's dominant time constant in s, `-1 / lambda` for `A = [[-0.2]]` (module documentation).
const PLANT_TIME_CONSTANT: f64 = 5.0;
/// The plant's gain from the heater in degC per % (module documentation).
const PLANT_GAIN: f64 = 0.2;
/// The fields of the runtime's experiment record (E5.6) that a finished experiment no longer changes.
const RECORD_KEYS: [&str; 4] = ["experiment_id", "status", "stop_reason", "restoration"];
/// How often the tests look at a running program, the journal and the runtime.
const POLL: Duration = Duration::from_millis(25);
/// The longest a verb may run. At ten times real time the longest takes seconds; this only ends a hung run.
const RUN_LIMIT: Duration = Duration::from_secs(180);
/// The longest the interruption test waits for the experiment to run in the runtime.
const EXPERIMENT_LIMIT: Duration = Duration::from_secs(120);

/// The failed checks of a journey. A failed check is recorded and the journey goes on.
#[derive(Default)]
struct Findings {
    failed: Vec<String>,
}

impl Findings {
    /// Records `finding` unless `holds`.
    fn check(&mut self, holds: bool, finding: impl Into<String>) {
        if !holds {
            self.failed.push(finding.into());
        }
    }

    /// A JSON file, or `Value::Null` and a finding when it cannot be read.
    fn json(&mut self, path: &Path) -> Value {
        let parsed = fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|text| {
                serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())
            });
        parsed.unwrap_or_else(|reason| {
            self.failed
                .push(format!("{} cannot be read: {reason}", path.display()));
            Value::Null
        })
    }

    /// Fails the test with every finding.
    fn conclude(&self) {
        assert!(
            self.failed.is_empty(),
            "{} check(s) failed:\n- {}",
            self.failed.len(),
            self.failed.join("\n- ")
        );
    }
}

/// A folder of one test with a copy of the example in it. The folder is removed when the test ends, even when it fails.
struct Scratch {
    folder: PathBuf,
    project: PathBuf,
    _remove: RemoveDir,
}

impl Scratch {
    fn new(name: &str) -> Self {
        Self::copy_of(name, EXAMPLE)
    }

    /// A folder of one test with a copy of `example` in it.
    fn copy_of(name: &str, example: &str) -> Self {
        let folder =
            std::env::temp_dir().join(format!("trust-cs-id-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let remove = RemoveDir(folder.clone());
        let project = folder.join("project");
        copy_dir(Path::new(example), &project);
        Self {
            folder,
            project,
            _remove: remove,
        }
    }
}

/// A copy of the example with its runtime running at ten times real time, and the places the verbs work in.
struct Lab {
    /// The runtime process. It is declared first, so it ends before the folder goes.
    _runtime: Runtime,
    scratch: Scratch,
    /// The folder of the operation journals, passed as `--journal-dir`.
    journal: PathBuf,
    /// The runtime's log, for the messages of a failed test.
    log: PathBuf,
    /// The runtime's control endpoint and token.
    connection: RuntimeConnection,
    /// The loop as Control Studio opens it.
    loop_definition: LoopDefinition,
    /// The program revision of the running session.
    program_revision: String,
}

impl Lab {
    /// Copies lesson 30, sets its speed, builds it, starts the runtime and waits for the first scan.
    fn start(name: &str) -> Self {
        Self::start_example(name, EXAMPLE, LOOP_ID)
    }

    /// Copies the example at `example`, sets its speed, builds it, opens its loop `loop_id`, starts the runtime and waits for
    /// the first scan.
    fn start_example(name: &str, example: &str, loop_id: &str) -> Self {
        let scratch = Scratch::copy_of(name, example);
        let project = &scratch.project;
        let socket = project.join(".control.sock");
        assert!(
            socket.as_os_str().len() <= 100,
            "the socket path {} is too long; use a shorter TMPDIR",
            socket.display()
        );
        prepare_runtime_toml(project, &socket);
        set_time_scale(project, TIME_SCALE);
        run_trust_runtime(&["build", "--project", &project.display().to_string()]);
        let loop_definition = loops::open(project, RUNTIME_ID, loop_id)
            .expect("Control Studio opens the example's loop");

        let log = project.join("runtime.log");
        let runtime = start_runtime(project, &log);
        let connection = RuntimeConnection {
            endpoint: format!("unix://{}", socket.display()),
            token: Some(TOKEN.into()),
        };
        let first = wait_for_status(&connection, &log);
        assert_eq!(
            first.simulation_time_scale, TIME_SCALE,
            "the runtime runs at the speed of simulation.toml"
        );
        // The first scan publishes the I/O snapshot, which makes the target a simulated plant (E1.3). Until then a live
        // verb is refused as a write to a physical target (A2.5).
        let mut client = RuntimeClient::connect(&connection).expect("connected");
        let bindings = loop_inspection_bindings(&loop_definition);
        inspect_until_available(
            &mut client,
            &loop_definition,
            &bindings,
            &first.program_revision,
        );
        let status = client.status().expect("status");
        assert_eq!(
            status.source(),
            EvidenceSource::SimulatedPlant,
            "a simulated runtime with simulated I/O is a simulated plant: {status:?}"
        );

        let journal = scratch.folder.join("journal");
        Self {
            _runtime: runtime,
            scratch,
            journal,
            log,
            connection,
            loop_definition,
            program_revision: first.program_revision,
        }
    }

    /// The arguments of `verb` on this lab's loop: the project, the loop and the journal folder, then `extra`.
    fn verb(&self, verb: &str, extra: &[&str]) -> Vec<String> {
        let mut arguments = strings(&[verb, "--project"]);
        arguments.push(self.scratch.project.display().to_string());
        arguments.extend(strings(&[
            "--loop",
            self.loop_definition.id.as_str(),
            "--journal-dir",
        ]));
        arguments.push(self.journal.display().to_string());
        arguments.extend(strings(extra));
        arguments
    }

    /// Starts `identify` with `extra` arguments and the control token; its output goes to files named after `name`.
    fn start_identify(&self, name: &str, extra: &[&str]) -> Program {
        Program::start(
            &self.verb("identify", extra),
            true,
            &self.scratch.folder,
            name,
        )
    }

    /// The controller command of the plant, read from the runtime: the loop's manipulated variable (E3).
    fn controller_command(&self) -> f64 {
        let mut client = RuntimeClient::connect(&self.connection).expect("connected");
        let bindings = loop_inspection_bindings(&self.loop_definition);
        let sample = inspect_until_available(
            &mut client,
            &self.loop_definition,
            &bindings,
            &self.program_revision,
        );
        sample
            .usable("manipulatedVariable")
            .unwrap_or_else(|| panic!("the controller command cannot be read: {sample:?}"))
    }

    /// The identifier of the experiment that the loop's journal holds, while an identification runs (E11.2).
    fn journaled_experiment(&self) -> Option<String> {
        Journal::open(
            &self.journal,
            &self.loop_definition.project_id,
            &self.loop_definition.id,
        )
        .ok()?
        .entries()
        .iter()
        .find(|entry| matches!(entry, JournalEntry::Experiment { .. }))
        .map(|entry| entry.operation_id().to_string())
    }

    /// Checks that the runtime reports no running experiment: the record of `id` has ended and its original command is
    /// restored (E5.4, E5.7). Returns the record when the runtime gave one.
    fn check_ended_and_restored(&self, findings: &mut Findings, id: &str) -> Option<Value> {
        match RuntimeClient::read_experiment_status(&self.connection, id) {
            StatusRead::Answer(record) => {
                findings.check(
                    record["status"].as_str() != Some("running"),
                    format!("experiment {id} should have ended: {record}"),
                );
                findings.check(
                    matches!(
                        record["restoration"].as_str(),
                        Some("restored" | "not_required")
                    ),
                    format!("experiment {id} should have restored its command: {record}"),
                );
                Some(record)
            }
            other => {
                findings.check(
                    false,
                    format!(
                        "the runtime should hold a readable record of experiment {id}: {other:?}"
                    ),
                );
                None
            }
        }
    }

    /// Checks that the loop's journal has no entry: no operation of the loop is running or unconfirmed (E11.6).
    fn check_journal_empty(&self, findings: &mut Findings, when: &str) {
        match Journal::open(
            &self.journal,
            &self.loop_definition.project_id,
            &self.loop_definition.id,
        ) {
            Ok(journal) => findings.check(
                journal.entries().is_empty(),
                format!(
                    "the journal should be empty {when}: {:?}",
                    journal.entries()
                ),
            ),
            Err(failure) => findings.check(
                false,
                format!("the journal should be readable {when}: {failure:?}"),
            ),
        }
    }

    /// Checks that the controller command is what it was before the verb ran.
    fn check_command_is(&self, findings: &mut Findings, original: f64) {
        let command = self.controller_command();
        findings.check(
            (command - original).abs() < 1e-6,
            format!(
                "the controller output should be back at its original command {original} %, it reads {command} %"
            ),
        );
    }

    /// Waits until the experiment of `program` runs in the runtime: the journal holds it before it is dispatched (E11.2), and
    /// the runtime's record says `running` (E5.6). Returns its identifier.
    fn wait_until_running(&self, program: &mut Program) -> String {
        let begun = Instant::now();
        let mut journaled: Option<String> = None;
        loop {
            if let Some(status) = program.child.try_wait().expect("poll the program") {
                panic!(
                    "the program ended before its experiment ran:\n{}",
                    self.describe(program, status)
                );
            }
            journaled = journaled.or_else(|| self.journaled_experiment());
            if let Some(id) = &journaled {
                let record = RuntimeClient::read_experiment_status(&self.connection, id);
                if matches!(&record, StatusRead::Answer(answer) if answer["status"] == "running") {
                    return id.clone();
                }
            }
            assert!(
                begun.elapsed() < EXPERIMENT_LIMIT,
                "no experiment ran within {} s; the program reports:\n{}",
                EXPERIMENT_LIMIT.as_secs(),
                program.stderr()
            );
            sleep(POLL);
        }
    }

    /// The end of the runtime's log.
    fn log_tail(&self) -> String {
        let text = fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        lines[lines.len().saturating_sub(40)..].join("\n")
    }

    /// What a failed test shows about a program that ended with `status`: its streams and the runtime's log.
    fn describe(&self, program: &Program, status: ExitStatus) -> String {
        format!(
            "exit status: {status}\nstandard output (start):\n{}\nstandard error:\n{}\nruntime log (end):\n{}",
            program.stdout().chars().take(3_000).collect::<String>(),
            program.stderr(),
            self.log_tail()
        )
    }
}

/// A run of the program under test, with its output in files so that nothing blocks on a full pipe. Dropping it ends the
/// process if it still runs, so a failed test leaves none behind.
struct Program {
    child: Child,
    out: PathBuf,
    err: PathBuf,
}

impl Program {
    /// Starts `trust-control-studio` with `arguments`. With `with_token` the runtime's control token is in the environment
    /// (§0.1b: never an argument); otherwise the variable is removed, so a value of the developer's never gets in.
    fn start(arguments: &[String], with_token: bool, folder: &Path, name: &str) -> Self {
        let out = folder.join(format!("{name}.out"));
        let err = folder.join(format!("{name}.err"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_trust-control-studio"));
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(File::create(&out).expect("the program's output file"))
            .stderr(File::create(&err).expect("the program's error file"));
        if with_token {
            command.env("TRUST_CONTROL_TOKEN", TOKEN);
        } else {
            command.env_remove("TRUST_CONTROL_TOKEN");
        }
        let child = command.spawn().expect("start the program");
        Self { child, out, err }
    }

    /// What the program has written to standard output so far.
    fn stdout(&self) -> String {
        read_text(&self.out)
    }

    /// What the program has written to standard error so far.
    fn stderr(&self) -> String {
        read_text(&self.err)
    }

    /// Waits for the program to end.
    fn wait(&mut self, limit: Duration) -> ExitStatus {
        self.wait_polling(limit, || {})
    }

    /// Waits for the program to end and calls `between` at every look, until `limit` has passed.
    fn wait_polling(&mut self, limit: Duration, mut between: impl FnMut()) -> ExitStatus {
        let begun = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().expect("poll the program") {
                return status;
            }
            between();
            assert!(
                begun.elapsed() < limit,
                "the program did not end within {} s; it reports:\n{}",
                limit.as_secs(),
                self.stderr()
            );
            sleep(POLL);
        }
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Sets `[simulation] time_scale` of the copy; the copy must have exactly one such line.
fn set_time_scale(project: &Path, scale: u32) {
    let path = project.join("simulation.toml");
    let text = fs::read_to_string(&path).expect("simulation.toml");
    let mut replaced = 0_u32;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.split('=').next().map(str::trim) == Some("time_scale") {
                replaced += 1;
                format!("time_scale = {scale}")
            } else {
                line.to_string()
            }
        })
        .collect();
    assert_eq!(
        replaced, 1,
        "simulation.toml has exactly one time_scale line"
    );
    fs::write(&path, format!("{}\n", lines.join("\n"))).expect("write simulation.toml");
}

/// Sends `SIGINT` once to the process `pid`, as a terminal sends Ctrl-C to the program in its foreground. It uses `kill`
/// and, where there is no such executable, the shell's own.
fn interrupt(pid: u32) {
    let pid = pid.to_string();
    let status = Command::new("kill")
        .args(["-INT", pid.as_str()])
        .status()
        .or_else(|_| {
            Command::new("sh")
                .args(["-c", &format!("kill -INT {pid}")])
                .status()
        })
        .expect("send SIGINT to the program");
    assert!(status.success(), "kill -INT {pid} failed: {status}");
}

/// The text of a file, or nothing when it cannot be read.
fn read_text(path: &Path) -> String {
    String::from_utf8_lossy(&fs::read(path).unwrap_or_default()).into_owned()
}

/// The arguments as owned strings.
fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| String::from(*part)).collect()
}

/// The `result` of the answer that a verb printed with `--json` (§0.1b: `{"result": …}`), or `Value::Null` and a finding that
/// shows what the program and the runtime said.
fn result_of(findings: &mut Findings, lab: &Lab, program: &Program, status: ExitStatus) -> Value {
    let answer = serde_json::from_str::<Value>(&program.stdout());
    if let Some(result) = answer.as_ref().ok().and_then(|answer| answer.get("result")) {
        result.clone()
    } else {
        findings.check(
            false,
            format!(
                "standard output should be one JSON value with a result:\n{}",
                lab.describe(program, status)
            ),
        );
        Value::Null
    }
}

/// The status word of `stage` in the stages of an answer (§4.2), for example `complete` or `needs-data`.
fn stage_status<'a>(answer: &'a Value, stage: &str) -> Option<&'a str> {
    answer["stages"]
        .as_array()?
        .iter()
        .find(|entry| entry["stage"] == stage)
        .and_then(|entry| entry["status"].as_str())
}

/// Checks the study on disk after a live identification: an Identify section that names the kept recording, and that
/// recording is complete evidence of the simulated plant (A8.3, A9, B10.3). Returns the section.
fn check_study_keeps_the_model(findings: &mut Findings, lab: &Lab, recording_id: &str) -> Value {
    let directory = storage::study_directory(&lab.scratch.project, &lab.loop_definition.id);
    let study = findings.json(&directory.join("study.json"));
    let section = study["sections"]["identify"].clone();
    findings.check(
        section.is_object(),
        "study.json should hold an identify section",
    );
    findings.check(
        section["estimationRecordingId"].as_str() == Some(recording_id),
        format!(
            "the section should name the recording {recording_id} that the result names, it names {}",
            section["estimationRecordingId"]
        ),
    );
    findings.check(
        section["provisional"].as_bool() == Some(true),
        "a fit from one recording should be provisional (B3.5)",
    );
    findings.check(
        section["source"].as_str() == Some("simulated-plant"),
        format!(
            "evidence from a simulated runtime should be labelled so (A9): {}",
            section["source"]
        ),
    );
    let listed: Vec<&str> = study["recordings"].as_array().map_or_else(Vec::new, |ids| {
        ids.iter().filter_map(Value::as_str).collect()
    });
    findings.check(
        listed == [recording_id],
        format!("the study should list its one recording, it lists {listed:?}"),
    );

    let recording = findings.json(
        &directory
            .join("recordings")
            .join(format!("{recording_id}.json")),
    );
    findings.check(
        recording["evidence"]["completeness"].as_str() == Some("complete"),
        format!(
            "the kept recording should be complete evidence (B10.3): {}",
            recording["evidence"]
        ),
    );
    findings.check(
        recording["source"].as_str() == Some("simulated-plant"),
        "the kept recording should come from the simulated plant (A9)",
    );
    section
}

/// Checks that the model of the result is the heater's: first order, with a positive gain and a time constant near the
/// plant's (module documentation), and that the result is labelled provisional (B3.5, B3.8).
fn check_heater_model(findings: &mut Findings, result: &Value, section: &Value) {
    let model_type = result["modelType"].as_str().unwrap_or_default();
    findings.check(
        model_type.starts_with("First-order"),
        format!("the heater has one state, so its model should be first order (with its delay), it is {model_type:?}"),
    );
    findings.check(
        result["provisional"].as_bool() == Some(true),
        "the result should be provisional (B3.5)",
    );
    for field in ["equation", "keyNumbers"] {
        findings.check(
            result[field].as_str().is_some_and(|text| !text.is_empty()),
            format!("the result should state its {field}"),
        );
    }
    let text = result["text"].as_str().unwrap_or_default();
    findings.check(
        text.contains("Provisional"),
        format!("the readable text should label the model Provisional (B3.5): {text:?}"),
    );

    let empty = Vec::new();
    let parameters = section["presentation"]["parameters"]
        .as_array()
        .unwrap_or(&empty);
    let named = |name: &str| {
        parameters
            .iter()
            .find(|parameter| parameter["name"] == name)
            .and_then(|parameter| parameter["value"].as_f64())
    };
    let gain = named("Gain");
    findings.check(
        gain.is_some_and(|gain| gain > 0.0),
        format!("the heater's gain should be positive, it is {gain:?}"),
    );
    findings.check(
        gain.is_some_and(|gain| (PLANT_GAIN * 0.75..=PLANT_GAIN * 1.25).contains(&gain)),
        format!(
            "the gain should be within 25 % of the plant's {PLANT_GAIN} degC per %, it is {gain:?}"
        ),
    );
    let time_constant = named("Time constant 1");
    findings.check(
        time_constant.is_some_and(|tau| {
            (PLANT_TIME_CONSTANT / 3.0..=PLANT_TIME_CONSTANT * 3.0).contains(&tau)
        }),
        format!("the time constant should be within a factor of three of the plant's {PLANT_TIME_CONSTANT} s, it is {time_constant:?}"),
    );
    findings.check(
        named("Time constant 2").is_none(),
        "a first-order model has one time constant",
    );
}

/// Checks that the plot of the result shows the applied plant input, the measurement and the model's prediction on one time
/// base (B3.8).
fn check_plot(findings: &mut Findings, result: &Value) {
    let plot = &result["plot"];
    let points = plot["timeSeconds"].as_array().map_or(0, Vec::len);
    findings.check(
        points >= 2,
        format!("the plot should show the recording, it shows {points} points"),
    );
    for role in ["plant-input", "measurement", "prediction"] {
        let trace = plot["traces"]
            .as_array()
            .and_then(|traces| traces.iter().find(|trace| trace["role"] == role));
        findings.check(
            trace.is_some_and(|trace| trace["values"].as_array().map(Vec::len) == Some(points)),
            format!("the plot should have a {role} trace with one value per time ({points})"),
        );
    }
}

/// Checks that after a reset the working study has no Identify section, and the previous study with its recording is in
/// `history/` (A8.4).
fn check_study_was_reset(findings: &mut Findings, lab: &Lab, recording_id: &str) {
    let directory = storage::study_directory(&lab.scratch.project, &lab.loop_definition.id);
    let working = directory.join("study.json");
    if working.is_file() {
        let study = findings.json(&working);
        findings.check(
            study["sections"].get("identify").is_none(),
            "the new working study should have no identify section",
        );
        findings.check(
            study["recordings"].as_array().map(Vec::len) == Some(0),
            "the new working study should list no recording",
        );
    }
    let mut history: Vec<PathBuf> = fs::read_dir(directory.join("history"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default();
    history.sort();
    findings.check(
        history.len() == 1,
        format!("history/ should hold the previous study, it holds {history:?}"),
    );
    if let Some(previous_folder) = history.first() {
        let previous = findings.json(&previous_folder.join("study.json"));
        findings.check(
            previous["sections"]["identify"]["estimationRecordingId"].as_str()
                == Some(recording_id),
            "the previous study should keep its Identify section",
        );
        let kept = previous_folder
            .join("recordings")
            .join(format!("{recording_id}.json"));
        findings.check(
            kept.is_file(),
            format!(
                "the recording is never deleted (A8.4), so {} should exist",
                kept.display()
            ),
        );
    }
}

/// Identify, reopen the kept recording, and reset, each through the program, on a heater at ten times real time (checklist
/// V1.6).
///
/// 1. `identify --json` runs a live campaign with Automatic: the loop is in Manual with a Manual output, so B3.3 gives a
///    plant-input step. It must end with status 0 and a provisional first-order model near the plant's (module
///    documentation). The study holds the Identify section and the complete recording that it names; the journal is empty;
///    the runtime reports no running experiment, and the controller output is back at its original command.
/// 2. `identify --recording <id> --json` fits the kept recording again: the same family, provisional, and no experiment is
///    started (the runtime keeps one experiment record, and a new start would replace it).
/// 3. `identify --reset --json` answers with the stages, and the study has no Identify section any more. The previous study
///    and its recording are in `history/`. The loop file, which holds the signals and the limits, is unchanged.
///
/// Run time at ten times real time, estimated from the plant and the loop file (`durationSeconds` is 60): the baseline
/// window is the longest of 20 samples and 5 % of 60 s, so 3 s, and the hold is at most 60 s - 3 s = 57 s. The response
/// settles after the 1 s delay and about 4 x 5 s = 20 s, so the hold is finished early at about 21 s of process time. The
/// verb then takes (3 s + 21 s) / 10 = 2.4 s of wall time, up to (3 s + 57 s) / 10 = 6 s if it runs to its end, plus about a
/// second to stop, keep and fit the recording. Reopening and resetting take well under a second each. With the start of the
/// runtime (a few seconds) and the build of the project, the test takes about 15 to 30 s.
#[test]
#[allow(clippy::too_many_lines)] // One journey, verb after verb; split, it would hide the order and the state each verb leaves.
fn identify_then_reopen_then_reset_through_the_command_line() {
    let mut findings = Findings::default();
    let lab = Lab::start("journey");
    let loop_file = fs::read(lab.scratch.project.join("control-loop.json")).expect("the loop file");
    let original_command = lab.controller_command();
    findings.check(
        (original_command - HOLDING_COMMAND).abs() < 1e-6,
        format!("the heater should hold {HOLDING_COMMAND} % in Manual before the step, it reads {original_command} %"),
    );

    // 1. Identify with Automatic. The experiment's identifier is read from the journal while it runs, because nothing but the
    // journal names it (E11.2) and the runtime has no list of experiments.
    let mut identify = lab.start_identify("identify", &["--json"]);
    let mut experiment = None;
    let status = identify.wait_polling(RUN_LIMIT, || {
        if experiment.is_none() {
            experiment = lab.journaled_experiment();
        }
    });
    findings.check(
        status.code() == Some(0),
        format!(
            "identify should finish with its result (status 0):\n{}",
            lab.describe(&identify, status)
        ),
    );
    findings.check(
        !identify.stderr().trim().is_empty(),
        "each progress update should be a line on standard error (§0.1b)",
    );
    let result = result_of(&mut findings, &lab, &identify, status);
    let recording_id = result["recordingId"]
        .as_str()
        .filter(|id| is_recording_id(id))
        .map(str::to_string);
    findings.check(
        recording_id.is_some(),
        format!(
            "the result should name a recording file of the study (A8.3), it names {}",
            result["recordingId"]
        ),
    );
    findings.check(
        stage_status(&result, "Identify") == Some("complete"),
        format!(
            "the Identify stage should be complete after the result, it is {:?}",
            stage_status(&result, "Identify")
        ),
    );
    check_plot(&mut findings, &result);
    let section = check_study_keeps_the_model(
        &mut findings,
        &lab,
        recording_id.as_deref().unwrap_or("(none)"),
    );
    check_heater_model(&mut findings, &result, &section);
    lab.check_journal_empty(&mut findings, "once the identification has settled");
    findings.check(
        experiment.is_some(),
        "the journal should hold the experiment while it runs (E11.2)",
    );
    let record = experiment
        .as_deref()
        .and_then(|id| lab.check_ended_and_restored(&mut findings, id));
    findings.check(
        record
            .as_ref()
            .is_some_and(|record| record["status"] == "completed"),
        format!("the step should end normally (status completed): {record:?}"),
    );
    lab.check_command_is(&mut findings, original_command);

    // 2. Reopen the kept recording: the same fit without an experiment. It needs the recording of a finished identification.
    let Some(recording_id) = recording_id.filter(|_| status.code() == Some(0)) else {
        findings.conclude();
        return;
    };
    let mut reopen = lab.start_identify("reopen", &["--recording", &recording_id, "--json"]);
    let status = reopen.wait(RUN_LIMIT);
    findings.check(
        status.code() == Some(0),
        format!(
            "identify --recording should finish with its result (status 0):\n{}",
            lab.describe(&reopen, status)
        ),
    );
    let reopened = result_of(&mut findings, &lab, &reopen, status);
    findings.check(
        reopened["modelType"] == result["modelType"],
        format!(
            "the family should stay the same: {} then {}",
            result["modelType"], reopened["modelType"]
        ),
    );
    findings.check(
        reopened["provisional"].as_bool() == Some(true),
        "the reopened result should be provisional",
    );
    findings.check(
        reopened["recordingId"].as_str() == Some(recording_id.as_str()),
        format!(
            "the reopened result should name the recording {recording_id}, it names {}",
            reopened["recordingId"]
        ),
    );
    if let (Some(id), Some(before)) = (experiment.as_deref(), record.as_ref()) {
        let after = RuntimeClient::read_experiment_status(&lab.connection, id);
        let unchanged = matches!(&after, StatusRead::Answer(current)
            if RECORD_KEYS.iter().all(|key| current[*key] == before[*key]));
        findings.check(
            unchanged,
            format!("the runtime should still hold the same experiment record, so no new experiment was started: {after:?}, before {before}"),
        );
    }
    lab.check_journal_empty(&mut findings, "after the recording was opened");

    // 3. Reset: a new working study, the previous one in history, the loop untouched.
    let mut reset = lab.start_identify("reset", &["--reset", "--json"]);
    let status = reset.wait(RUN_LIMIT);
    findings.check(
        status.code() == Some(0),
        format!(
            "identify --reset should finish with its result (status 0):\n{}",
            lab.describe(&reset, status)
        ),
    );
    let cleared = result_of(&mut findings, &lab, &reset, status);
    findings.check(
        stage_status(&cleared, "Identify") == Some("needs-data"),
        format!(
            "the stages of the new working study should show Identify as needs-data, it is {:?}",
            stage_status(&cleared, "Identify")
        ),
    );
    check_study_was_reset(&mut findings, &lab, &recording_id);
    let loop_file_now = fs::read(lab.scratch.project.join("control-loop.json")).ok();
    findings.check(
        loop_file_now.as_ref() == Some(&loop_file),
        "the loop file, which holds the signals and the limits, should be unchanged",
    );
    lab.check_journal_empty(&mut findings, "after the reset");
    findings.conclude();
}

/// One Ctrl-C while the experiment runs cancels the identification and restores the plant (§0.1b, A8.6, E5.5). The test waits
/// until the runtime reports the experiment running, then sends `SIGINT` once to the program. The program must end with
/// status 130 (the interruption ended the verb, §0.1b; a restoration that is not confirmed would end with 1 and fail this
/// test), the runtime must report the experiment ended with its command restored, the controller output must be back at
/// its original command, and the journal must hold no entry. No model is kept. The second Ctrl-C, which leaves everything
/// to the runtime and the journal, is not tested here.
///
/// Run time at ten times real time: the baseline takes 3 s / 10 = 0.3 s, the press comes within about 0.1 s of the
/// experiment's start, and the program aborts the experiment within 0.1 s more; the runtime restores it within three
/// executions of the task (0.03 s). With the start of the runtime and the build of the project the test takes 10 to 20 s.
#[test]
fn interrupt_once_cancels_and_restores() {
    let lab = Lab::start("interrupt");
    let original_command = lab.controller_command();
    let mut identify = lab.start_identify("identify", &["--json"]);
    let experiment = lab.wait_until_running(&mut identify);

    interrupt(identify.child.id());
    let status = identify.wait(RUN_LIMIT);
    let mut findings = Findings::default();
    findings.check(
        status.code() == Some(130),
        format!(
            "one Ctrl-C should end the verb with status 130 (cancelled):\n{}",
            lab.describe(&identify, status)
        ),
    );
    lab.check_ended_and_restored(&mut findings, &experiment);
    lab.check_command_is(&mut findings, original_command);
    lab.check_journal_empty(
        &mut findings,
        "once the cancelled identification has settled",
    );
    let working =
        storage::study_directory(&lab.scratch.project, &lab.loop_definition.id).join("study.json");
    if working.is_file() {
        let study = findings.json(&working);
        findings.check(
            study["sections"].get("identify").is_none(),
            "a cancelled identification should keep no model",
        );
    }
    findings.conclude();
}
