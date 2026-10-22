//! The harness that the two identification files of checklist V1.7 share, `control_studio_identify_methods.rs` and
//! `control_studio_identify_continuity.rs`: a copy of an example project whose runtime runs `trust-runtime run --simulation`
//! at ten times real time, the built `trust-control-studio` program run on that copy, and the checks of a result, of the
//! study on disk and of the journal.
//!
//! `control_studio_identify.rs` (V1.6) keeps its own copy of the same kind of helpers: it is a test crate of its own, and this
//! stage does not change it. The runtime helpers are those of `tests/common/mod.rs`, which each including file declares as
//! `mod common;` and which this module reads as `crate::common`.
//!
//! This module takes no decision of the product. It reads files and the runtime's answers, and a failed check is a finding:
//! a test fails with every finding of its run. Both including files use every item here, so the module needs no
//! `allow(dead_code)`; an item that only one of them uses belongs in that file instead.

// Each test crate includes this module and uses a part of it.
#![allow(dead_code)]

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::Value;
use trust_control::experiment::ControlExperimentPlan;
use trust_control::identification::reconcile::StatusRead;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::loop_inspection_bindings;
use trust_control::study::is_recording_id;
use trust_control_studio::journal::{Journal, JournalEntry};
use trust_control_studio::runtime_client::RuntimeClient;
use trust_control_studio::{loops, storage};

use crate::common::{
    copy_dir, inspect_until_available, prepare_runtime_toml, run_trust_runtime, start_runtime,
    wait_for_status, RemoveDir, Runtime, TOKEN,
};

/// The heater of lesson 30, `examples/control/06-pid-tuning-methods/30-model-based-pi-tuning`. Its program starts in Manual with
/// the output at 75 % (`src/Main.st`: `AutoMode := FALSE`, `ManualOutput := 75.0`), so the open-loop methods run in its own mode.
pub const LESSON_30: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning"
);
/// The identifier of lesson 30's loop (`control-loop.json`).
pub const LESSON_30_LOOP: &str = "control-model-based-tuning";
/// The runtime that the verbs name for a project's loops; every example's loop is bound to it.
const RUNTIME_ID: &str = "simulator";
/// The speed of the simulation, ten times real time (the speed the user chose for lessons).
const TIME_SCALE: u32 = 10;
/// The command that the heater holds in Manual before and after an experiment (`ManualOutput := LREAL#75.0`).
pub const HOLDING_COMMAND: f64 = 75.0;
/// The longest a verb may run. At ten times real time the longest run takes seconds; this only ends a hung run.
pub const RUN_LIMIT: Duration = Duration::from_secs(180);
/// How often a test looks at a running program, the journal and the runtime.
const POLL: Duration = Duration::from_millis(25);

/// The failed checks of a journey. A failed check is recorded and the journey goes on, so one run reports every failure that
/// does not depend on another.
#[derive(Default)]
pub struct Findings {
    failed: Vec<String>,
}

impl Findings {
    /// Records `finding` unless `holds`.
    pub fn check(&mut self, holds: bool, finding: impl Into<String>) {
        if !holds {
            self.failed.push(finding.into());
        }
    }

    /// A JSON file, or `Value::Null` and a finding when it cannot be read.
    pub fn json(&mut self, path: &Path) -> Value {
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
    pub fn conclude(&self) {
        assert!(
            self.failed.is_empty(),
            "{} check(s) failed:\n- {}",
            self.failed.len(),
            self.failed.join("\n- ")
        );
    }
}

/// A folder of one test with a copy of an example in it. The folder is removed when the test ends, even when it fails.
pub struct Scratch {
    /// The folder. The program's journals and its output files are written beside the copy.
    pub folder: PathBuf,
    /// The copy of the example: the project that the verbs work on.
    pub project: PathBuf,
    _remove: RemoveDir,
}

impl Scratch {
    /// A folder named after `name`, holding a copy of the example at `example`.
    pub fn copy_of(name: &str, example: &str) -> Self {
        let folder =
            std::env::temp_dir().join(format!("trust-cs-id-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let remove = RemoveDir(folder.clone());
        let project = folder.join("project");
        copy_dir(Path::new(example), &project);
        remove_test_programs(&project);
        Self {
            folder,
            project,
            _remove: remove,
        }
    }
}

/// Removes the copied sources that declare a `TEST_PROGRAM`. A lesson's ST unit tests run under `trust-dev test`, which
/// registers its test programs; an ordinary runtime build rejects an unconfigured test program (specification 22, test
/// discovery), so the runtime is given the lesson's program without them.
fn remove_test_programs(project: &Path) {
    let Ok(entries) = fs::read_dir(project.join("src")) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_structured_text = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("st"));
        let declares_test_program = is_structured_text
            && fs::read_to_string(&path).is_ok_and(|text| {
                text.lines()
                    .any(|line| line.trim_start().starts_with("TEST_PROGRAM"))
            });
        if declares_test_program {
            fs::remove_file(&path).expect("remove the copied test program");
        }
    }
}

/// A copy of an example with its runtime running at ten times real time, and the places that the verbs work in.
pub struct Lab {
    /// The runtime process. It is declared first, so it ends before the folder goes.
    _runtime: Runtime,
    /// The folder and the copy of the example.
    pub scratch: Scratch,
    /// The folder of the operation journals, passed as `--journal-dir`.
    pub journal: PathBuf,
    /// The runtime's log, for the messages of a failed test.
    log: PathBuf,
    /// The runtime's control endpoint and token.
    pub connection: RuntimeConnection,
    /// The loop as Control Studio opens it.
    pub loop_definition: LoopDefinition,
    /// The program revision of the running session.
    program_revision: String,
}

impl Lab {
    /// Copies the example at `example`, sets its speed, builds it, opens its loop `loop_id`, starts the runtime and waits for
    /// the first scan. The runtime answers on a socket in the copy, and the verbs receive the copy's token.
    pub fn start_example(name: &str, example: &str, loop_id: &str) -> Self {
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
            runtime_id: None,
        };
        let first = wait_for_status(&connection, &log);
        assert_eq!(
            first.simulation_time_scale, TIME_SCALE,
            "the runtime runs at the speed of simulation.toml"
        );
        // The first scan publishes the I/O snapshot, which makes the target a simulated plant (E1.3).
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
    pub fn verb(&self, verb: &str, extra: &[&str]) -> Vec<String> {
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

    /// Starts `verb` on this lab's loop with `extra` arguments and the control token in the environment (§0.1b: never an
    /// argument). Its output goes to files named after `name`.
    pub fn start_verb(&self, verb: &str, extra: &[&str], name: &str) -> Program {
        Program::start(&self.verb(verb, extra), true, &self.scratch.folder, name)
    }

    /// The controller command of the plant, read from the runtime: the loop's manipulated variable (E3).
    pub fn controller_command(&self) -> f64 {
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

    /// The reviewed plan of the experiment that the identification campaign of the loop's journal holds once the campaign has
    /// dispatched its step (E11.2). `None` while the journal holds no such step (B3.4).
    pub fn journaled_plan(&self) -> Option<ControlExperimentPlan> {
        let journal = Journal::open(
            &self.journal,
            &self.loop_definition.project_id,
            &self.loop_definition.id,
        )
        .ok()?;
        journal.entries().iter().find_map(|entry| match entry {
            JournalEntry::IdentificationCampaign { campaign } => campaign.plan.clone(),
            JournalEntry::RecordingOwner { .. }
            | JournalEntry::Experiment { .. }
            | JournalEntry::IsolatedJob { .. }
            | JournalEntry::Commissioning { .. }
            | JournalEntry::IdentificationMode { .. } => None,
        })
    }

    /// Checks that the runtime reports no running experiment: the record of `id` has ended and its command is restored (E5.4,
    /// E5.7). Returns the record when the runtime gave one.
    pub fn check_ended_and_restored(&self, findings: &mut Findings, id: &str) -> Option<Value> {
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
    pub fn check_journal_empty(&self, findings: &mut Findings, when: &str) {
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

    /// Checks that the controller command is what it was before the experiment: the output is back at `original`, its value
    /// before the verb ran (B3.4 step 5).
    pub fn check_command_is(&self, findings: &mut Findings, original: f64) {
        let command = self.controller_command();
        findings.check(
            (command - original).abs() < 1e-6,
            format!(
                "the controller output should be back at its original command {original} %, it reads {command} %"
            ),
        );
    }

    /// What a failed test shows about a program that ended with `status`: its streams and the end of the runtime's log.
    pub fn describe(&self, program: &Program, status: ExitStatus) -> String {
        format!(
            "exit status: {status}\nstandard output (start):\n{}\nstandard error:\n{}\nruntime log (end):\n{}",
            program.stdout().chars().take(3_000).collect::<String>(),
            program.stderr(),
            self.log_tail()
        )
    }

    /// The end of the runtime's log.
    fn log_tail(&self) -> String {
        let text = fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        lines[lines.len().saturating_sub(40)..].join("\n")
    }
}

/// A run of the program under test, with its output in files so that nothing blocks on a full pipe. Dropping it ends the
/// process if it still runs, so a failed test leaves none behind.
pub struct Program {
    /// The process. A test may end it outright with `kill`, which sends SIGKILL on Unix, as a crash would.
    pub child: Child,
    out: PathBuf,
    err: PathBuf,
}

impl Program {
    /// Starts `trust-control-studio` with `arguments`. With `with_token` the runtime's control token is in the environment
    /// (§0.1b: never an argument); otherwise the variable is removed, so a value of the developer's never gets in.
    pub fn start(arguments: &[String], with_token: bool, folder: &Path, name: &str) -> Self {
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
    pub fn stdout(&self) -> String {
        read_text(&self.out)
    }

    /// What the program has written to standard error so far.
    pub fn stderr(&self) -> String {
        read_text(&self.err)
    }

    /// Waits for the program to end.
    pub fn wait(&mut self, limit: Duration) -> ExitStatus {
        self.wait_polling(limit, || {})
    }

    /// Waits for the program to end and calls `between` at every look, until `limit` has passed.
    pub fn wait_polling(&mut self, limit: Duration, mut between: impl FnMut()) -> ExitStatus {
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

/// The `result` of the answer that a verb printed with `--json` (§0.1b: `{"result": …}`), or `Value::Null` and a finding that
/// shows what the program and the runtime said.
pub fn result_of(
    findings: &mut Findings,
    lab: &Lab,
    program: &Program,
    status: ExitStatus,
) -> Value {
    let answer = serde_json::from_str::<Value>(&program.stdout()).ok();
    if let Some(result) = answer.and_then(|answer| answer.get("result").cloned()) {
        result
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
pub fn stage_status<'a>(answer: &'a Value, stage: &str) -> Option<&'a str> {
    answer["stages"]
        .as_array()?
        .iter()
        .find(|entry| entry["stage"] == stage)
        .and_then(|entry| entry["status"].as_str())
}

/// The recording file that a result names (A8.3), when it names one; a finding when it names none.
pub fn result_recording_id(findings: &mut Findings, result: &Value) -> Option<String> {
    let id = result["recordingId"]
        .as_str()
        .filter(|id| is_recording_id(id))
        .map(str::to_string);
    findings.check(
        id.is_some(),
        format!(
            "the result should name a recording file of the study (A8.3), it names {}",
            result["recordingId"]
        ),
    );
    id
}

/// Checks the study on disk after a live identification: an Identify section that names the kept recording, and that recording
/// is complete evidence of the simulated plant (A8.3, A9, B10.3). Returns the section.
pub fn check_study_keeps_the_model(
    findings: &mut Findings,
    lab: &Lab,
    recording_id: &str,
) -> Value {
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

/// Checks that the working study of the loop holds no Identify section, so that no model is kept (A8.4, B3.9). A study that
/// was never written holds none either.
pub fn check_no_model_kept(findings: &mut Findings, lab: &Lab) {
    let working =
        storage::study_directory(&lab.scratch.project, &lab.loop_definition.id).join("study.json");
    if working.is_file() {
        let study = findings.json(&working);
        findings.check(
            study["sections"].get("identify").is_none(),
            "the study should keep no model: it has no identify section",
        );
    }
}

/// The value of the named parameter in an identification section's presentation (`presentation.parameters`, B3.8), when it has one.
pub fn parameter(section: &Value, name: &str) -> Option<f64> {
    let parameters = section["presentation"]["parameters"].as_array()?;
    let found = parameters.iter().find(|item| item["name"] == name)?;
    found["value"].as_f64()
}

/// Checks that the model of a result is a plausible model of the heater and that the result is provisional (B3.5, B3.8): its
/// family is first order (the heater has one state, with its transport delay), its gain is positive, and its readable text
/// labels it Provisional. The numbers of the fit are checked by the tests that check them.
pub fn check_plausible_model(findings: &mut Findings, result: &Value, section: &Value) {
    let model_type = result["modelType"].as_str().unwrap_or_default();
    findings.check(
        model_type.starts_with("First-order"),
        format!(
            "the heater has one state, so its model should be first order, with or without its delay; it is {model_type:?}"
        ),
    );
    findings.check(
        result["provisional"].as_bool() == Some(true),
        "the result should be provisional (B3.5)",
    );
    let text = result["text"].as_str().unwrap_or_default();
    findings.check(
        text.contains("Provisional"),
        format!("the readable text should label the model Provisional (B3.5): {text:?}"),
    );
    let gain = parameter(section, "Gain");
    findings.check(
        gain.is_some_and(|gain| gain > 0.0),
        format!("the heater's gain should be positive, it is {gain:?}"),
    );
}

/// Checks that the plot of the result shows the applied plant input, the measurement and the model's prediction on one time
/// base (B3.8).
pub fn check_plot(findings: &mut Findings, result: &Value) {
    let plot = &result["plot"];
    let points = plot["timeSeconds"].as_array().map_or(0, Vec::len);
    findings.check(
        points >= 2,
        format!("the plot should show the recording, it shows {points} points"),
    );
    for role in ["plant-input", "measurement", "prediction"] {
        let trace = plot["traces"]
            .as_array()
            .and_then(|series| series.iter().find(|item| item["role"] == role));
        findings.check(
            trace.is_some_and(|trace| trace["values"].as_array().map(Vec::len) == Some(points)),
            format!("the plot should have a {role} trace with one value per time ({points})"),
        );
    }
}

/// The `[simulation] time_scale` of the copy is set to `scale`; the copy must have exactly one such line.
fn set_time_scale(project: &Path, scale: u32) {
    let path = project.join("simulation.toml");
    let text = fs::read_to_string(&path).expect("simulation.toml");
    let mut replaced = 0_u32;
    let edited: Vec<String> = text
        .lines()
        .map(|row| {
            if row.split('=').next().map(str::trim) == Some("time_scale") {
                replaced += 1;
                format!("time_scale = {scale}")
            } else {
                row.to_string()
            }
        })
        .collect();
    assert_eq!(
        replaced, 1,
        "simulation.toml has exactly one time_scale line"
    );
    fs::write(&path, format!("{}\n", edited.join("\n"))).expect("write simulation.toml");
}

/// The text of a file, or nothing when it cannot be read.
fn read_text(path: &Path) -> String {
    String::from_utf8_lossy(&fs::read(path).unwrap_or_default()).into_owned()
}

/// The arguments as owned strings.
fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| String::from(*part)).collect()
}
