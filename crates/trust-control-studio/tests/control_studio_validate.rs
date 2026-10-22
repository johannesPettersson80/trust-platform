//! Control Studio's `validate` verb through the built program `trust-control-studio`, on a real runtime (stage V3, layer 3;
//! specification 35, §0.1b, B5.1 to B5.5, A8.4, A9, N3.4, N8.5 and N8.9; runtime protocol annex E8.3 and E8.4).
//!
//! The program runs on a copy of a heater lesson: lesson 85, `examples/control/06-pid-tuning-methods/85-tuning-to-requirements`,
//! and lesson 30, `30-model-based-pi-tuning`, whose program designs its own startup gains during its first scan, so its candidate
//! reaches the program only once the runtime writes it again at the task boundary after that scan (protocol annex E8.3).
//! `LESSON_85` and `LESSON_30` are the only places that name them; the loop and its plant are read from the copy's
//! `control-loop.json`, and the copy drops a lesson's `TEST_PROGRAM`, which an ordinary runtime build rejects (specification 22), against `trust-runtime run --simulation` at ten times real time
//! (`time_scale = 10` in the copy's `simulation.toml`, as `control_studio_tune.rs` sets it). Each test works on a copy of its
//! own. The control token goes to the program through `TRUST_CONTROL_TOKEN` (§0.1b), and every run names its own
//! `--journal-dir`.
//!
//! The first test needs no runtime. `validate` before any model is refused at once, with Identify as its next action, and it
//! creates no study (decision 3 and B5).
//!
//! The second test runs `identify --json`, then a refused `validate` before any candidate (next action Tune, with nothing stored
//! and no recording started), then `tune --method simc --json` (which never reaches the runtime), then `validate --json`. The
//! model is provisional after identify, so the model check runs (B5.2):
//! - if it passes, the verb answers with its result. The model check's recording is the Identify section's validation recording,
//!   a recording other than its estimation recording (N3.4), which the study lists and keeps. Every scenario runs its current
//!   settings and its candidate as isolated simulations on the configured plant, which drives the loop's signals (B5.3, E8.3);
//!   each run has its evidence and its status. Every requirement of every run has its result with value, limit, unit and
//!   source (B5.4). The overall result is the candidate's (N8.5), and Apply is available exactly when it passes; the verb exits
//!   with 0 exactly then, and with 1 otherwise (§0.1b);
//! - if it fails, the verb stops with the check's failure: its cause is the model or the data, its next action is Identify, and
//!   its text names the error and the limit (B5.5). The model stays provisional, so Apply stays unavailable (N3.5).
//!
//! In both cases the study keeps the Validate section, current for the model that Identify keeps (A8.4), and the candidate that
//! Tune keeps stays unapplied. The gains and the parameter revision of the live controller read the same before and after the
//! validation: nothing is written to the controller (N8.9). The parameter overrides of the candidate runs (B5.3) are not in the
//! answer, so this test does not check them.
//!
//! The answer's names are those of `ValidateResult` in `crates/trust-control/src/messages/validate_answer.rs`, and the checks read them as
//! written there: `modelCheck` (absent when no check ran; with `status`, `modelRevision`, `recordingId`, `limit` and
//! `normalizedError`), `scenarios` (each with `id`, `plant`, `current` and `candidate`, which are runs with `runId`, `evidence`,
//! `status` and `requirements`), the result's `status` (the candidate's overall result) and `applyAvailable`. A requirement has
//! `id`, `status`, `value` (absent when there is none), `limit`, `unit`, `source` and `reason`. The study's Validate section is
//! not written yet, so the names read from it are assumed: `status`, `applyAvailable`, `modelRevision` and `modelCheck`.
//!
//! Run time at ten times real time: the first test takes well under a second. The second takes about 15 to 40 s: the build of the
//! project and the start of the runtime take a few seconds, `identify` and the model check's live campaign each take a few
//! seconds of wall time (the step is held until the response settles, as in `control_studio_tune.rs`), and the isolated runs do
//! not follow the live speed (A10.5). Each of them is 60 s of simulated time on a 100 ms grid, which takes well under a second of
//! wall time, and there is one per scenario and side (current and candidate). The time has not been measured yet; the builder run
//! reports it.
//!
//! # Running
//!
//! `cargo test -p trust-control-studio --test control_studio_validate`. The test that starts a runtime runs the `trust-runtime`
//! executable of its own target folder, so build it first with `cargo build -p trust-runtime --bin trust-runtime`, or name another
//! one in `TRUST_RUNTIME_BIN`. The file is Unix only, as `control_studio_tune.rs` is: the runtime's endpoint is a Unix socket.
#![cfg(unix)]

mod common;

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::Value;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::{loop_inspection_bindings, SignalBinding};
use trust_control_studio::runtime_client::RuntimeClient;
use trust_control_studio::{loops, storage};

use common::{
    copy_dir, inspect_until_available, prepare_runtime_toml, run_trust_runtime, start_runtime,
    wait_for_status, RemoveDir, Runtime, DEADLINE, TOKEN,
};

/// Lesson 85's heater project (see the module documentation).
const LESSON_85: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/85-tuning-to-requirements"
);
/// Lesson 30's heater project, whose program designs its own startup gains (see the module documentation).
const LESSON_30: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning"
);
/// The runtime the verbs name for a project's loops; the example's loop is bound to it.
const RUNTIME_ID: &str = "simulator";
/// The speed of the simulation, ten times real time.
const TIME_SCALE: u32 = 10;
/// A control endpoint that nothing listens on: Tune must finish without reaching it (B4.1).
const UNREACHABLE: &str = "tcp://127.0.0.1:1";
/// How often a test looks at a running program.
const POLL: Duration = Duration::from_millis(25);
/// The longest `identify` may run; at ten times real time it takes seconds, so this only ends a hung run.
const RUN_LIMIT: Duration = Duration::from_secs(180);
/// The longest `validate` may run: the model check's live campaign, then the isolated runs.
const VALIDATE_LIMIT: Duration = Duration::from_secs(300);
/// The smallest difference between two gains that makes them different designs (the engine writes gains with eight
/// significant digits, so a gain that differs by less is the same value).
const GAIN_DIFFERENCE: f64 = 1e-6;
/// The case that every validation runs: the loop's target response (`CampaignCase`, `target-response`).
const TARGET_RESPONSE: &str = "target-response";
/// The proportional gain of `Control.PID` in the program: `Main.st` sets it at startup, and a validation must not change it.
const KP: &str = "P1.Parameters.ProportionalGain";
/// The live values a validation must leave alone, read through the runtime's own inspection (E3): the gains and
/// `ParameterRevision`, which `Main.st` counts up each time it replaces the parameter record.
const LIVE: [(&str, &str, &str); 3] = [
    ("kp", KP, "LREAL"),
    ("ki", "P1.Parameters.IntegralGain", "LREAL"),
    ("revision", "P1.ParameterRevision", "DINT"),
];

/// The failed checks of a test, and the JSON they read, which a failure shows. A failed check is recorded and the test goes on.
#[derive(Default)]
struct Findings {
    failed: Vec<String>,
    shown: String,
}

impl Findings {
    /// Records `finding` unless `holds`.
    fn check(&mut self, holds: bool, finding: impl Into<String>) {
        if !holds {
            self.failed.push(finding.into());
        }
    }

    /// Fails the test with every finding.
    fn conclude(&self) {
        assert!(
            self.failed.is_empty(),
            "{} check(s) failed:\n- {}\nthe JSON read:\n{}",
            self.failed.len(),
            self.failed.join("\n- "),
            self.shown
        );
    }
}

/// A folder of one test with a copy of the example in it, removed when the test ends, even when it fails.
struct Scratch {
    folder: PathBuf,
    project: PathBuf,
    _remove: RemoveDir,
}

impl Scratch {
    fn new(name: &str, example: &str) -> Self {
        let folder =
            std::env::temp_dir().join(format!("trust-cs-validate-{name}-{}", std::process::id()));
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

/// A run of the program under test, with its output in files so that nothing blocks on a full pipe. Dropping it ends the
/// process if it still runs, so a failed test leaves none behind.
struct Program {
    child: Child,
    out: PathBuf,
    err: PathBuf,
}

impl Program {
    /// Starts `trust-control-studio` with `arguments`. With `with_token` the control token is in the environment (§0.1b: never
    /// an argument); otherwise the variable is removed, so a value of the developer's never gets in.
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

    /// Waits for the program to end, for `limit` at most.
    fn wait(&mut self, limit: Duration) -> ExitStatus {
        let begun = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().expect("poll the program") {
                return status;
            }
            assert!(
                begun.elapsed() < limit,
                "the program did not end within {} s; it reports:\n{}",
                limit.as_secs(),
                read_text(&self.err)
            );
            sleep(POLL);
        }
    }

    /// The answer on standard output, as `--json` writes it: `{"result": …}` or `{"failure": …}`. `Value::Null` when the output
    /// is not JSON, which the checks then report.
    fn answer(&self) -> Value {
        serde_json::from_str(&read_text(&self.out)).unwrap_or_default()
    }

    /// What a failed test shows of the program that ended with `status`: its standard output (start) and standard error.
    fn report(&self, status: ExitStatus) -> String {
        let output: String = read_text(&self.out).chars().take(2_000).collect();
        format!(
            "{status}\nstandard output (start):\n{output}\nstandard error:\n{}",
            read_text(&self.err)
        )
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A copy of a lesson with its runtime running at ten times real time, its journal folder, and what the checks read from the
/// loop and from the live controller.
struct Lab {
    /// The runtime process. It is declared first, so it ends before the folder goes.
    _runtime: Runtime,
    scratch: Scratch,
    /// The folder of the operation journals, passed as `--journal-dir`.
    journal: PathBuf,
    connection: RuntimeConnection,
    loop_definition: LoopDefinition,
    program_revision: String,
}

impl Lab {
    /// Copies the lesson `example`, sets its speed, builds it, starts the runtime and waits for the first scan, which makes the
    /// target a simulated plant (E1.3) and so admits isolated runs (E8.1).
    fn start(name: &str, example: &str) -> Self {
        let scratch = Scratch::new(name, example);
        let project = scratch.project.clone();
        let socket = project.join(".control.sock");
        assert!(
            socket.as_os_str().len() <= 100,
            "the socket path {} is too long; use a shorter TMPDIR",
            socket.display()
        );
        prepare_runtime_toml(&project, &socket);
        set_time_scale(&project, TIME_SCALE);
        run_trust_runtime(&["build", "--project", &project.display().to_string()]);
        let loop_definition = loops::open(&project, RUNTIME_ID, &loop_id_of(&project))
            .expect("Control Studio opens the example's loop");

        let log = project.join("runtime.log");
        let runtime = start_runtime(&project, &log);
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
            connection,
            loop_definition,
            program_revision: first.program_revision,
        }
    }

    /// The live values that a validation must leave alone, read through the runtime's own inspection (E3).
    fn live_settings(&self) -> Vec<Option<f64>> {
        let bindings: Vec<SignalBinding> = LIVE
            .iter()
            .map(|(id, symbol, iec_type)| SignalBinding {
                id: (*id).into(),
                address: None,
                symbol: Some((*symbol).into()),
                iec_type: (*iec_type).into(),
                unit: None,
            })
            .collect();
        let mut client = RuntimeClient::connect(&self.connection).expect("connected");
        inspect_until_available(
            &mut client,
            &self.loop_definition,
            &bindings,
            &self.program_revision,
        )
        .values
    }

    /// Runs `verb` on the example's loop to its end, with `extra` arguments. With `with_token` the control token is in the
    /// environment. The program's output goes to files named after the verb.
    fn run(
        &self,
        verb: &str,
        extra: &[&str],
        with_token: bool,
        limit: Duration,
    ) -> (Program, ExitStatus) {
        let arguments = verb_arguments(verb, &self.scratch.project, &self.journal, extra);
        let mut program = Program::start(&arguments, with_token, &self.scratch.folder, verb);
        let status = program.wait(limit);
        (program, status)
    }

    /// Identifies the heater with `identify --json`, the loop's own method (Automatic: a plant-input step in Manual, B3.3), and
    /// returns the Identify section that the study keeps. Nothing after it can run without the model, so the test ends here when
    /// the verb does not finish with its result.
    fn identify(&self, findings: &mut Findings) -> Value {
        let (program, status) = self.run("identify", &["--json"], true, RUN_LIMIT);
        let section = study_of(&self.scratch.project)["sections"]["identify"].clone();
        let finished = status.code() == Some(0) && section.is_object();
        findings.check(
            finished,
            format!(
                "identify should finish with its result and keep its model:\n{}",
                program.report(status)
            ),
        );
        if !finished {
            findings.conclude();
        }
        section
    }

    /// Runs `tune --json` with `extra` arguments and without the control token: Tune never reaches the runtime (B4.1). Returns the
    /// `result` of its answer; the test ends here when the verb does not finish with one.
    fn tune(&self, findings: &mut Findings, extra: &[&str]) -> Value {
        let mut arguments = vec!["--json"];
        arguments.extend_from_slice(extra);
        let (program, status) = self.run("tune", &arguments, false, DEADLINE);
        let result = program.answer().get("result").cloned();
        findings.check(
            status.code() == Some(0) && result.is_some(),
            format!(
                "tune should finish with its result (status 0):\n{}",
                program.report(status)
            ),
        );
        if result.is_none() {
            findings.conclude();
        }
        result.unwrap_or_default()
    }

    /// Runs `validate --json` where it must be refused at once (decision 3): the answer is a failure whose next action is `next`,
    /// the verb exits with 1, and the study keeps exactly what it kept before, with no recording started.
    fn check_refused(&self, findings: &mut Findings, next: &str) {
        let before = study_of(&self.scratch.project);
        let (program, status) = self.run("validate", &["--json"], true, DEADLINE);
        let answer = program.answer();
        findings.check(
            status.code() == Some(1)
                && answer["failure"]["nextAction"] == next
                && answer.get("result").is_none(),
            format!(
                "validate is refused at once with next action {next} (§0.1b):\n{}",
                program.report(status)
            ),
        );
        let after = study_of(&self.scratch.project);
        findings.check(
            before["sections"] == after["sections"] && before["recordings"] == after["recordings"],
            "a refused validation stores nothing and starts no recording",
        );
    }
}

/// Removes the copied sources that declare a `TEST_PROGRAM`: an ordinary runtime build rejects an unconfigured test program
/// (specification 22), as `identify_lab` removes them.
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
    assert_eq!(replaced, 1, "simulation.toml has one time_scale line");
    fs::write(&path, format!("{}\n", lines.join("\n"))).expect("write simulation.toml");
}

fn read_text(path: &Path) -> String {
    String::from_utf8_lossy(&fs::read(path).unwrap_or_default()).into_owned()
}

/// The arguments of `verb` on the example's loop: the project, the loop and the journal folder, then `extra`.
fn verb_arguments(verb: &str, project: &Path, journal: &Path, extra: &[&str]) -> Vec<String> {
    let mut arguments = vec![verb.to_string(), "--project".to_string()];
    arguments.push(project.display().to_string());
    arguments.extend([
        "--loop".to_string(),
        loop_id_of(project),
        "--journal-dir".to_string(),
    ]);
    arguments.push(journal.display().to_string());
    arguments.extend(extra.iter().map(|argument| (*argument).to_string()));
    arguments
}

/// The copy's loop file, `control-loop.json`.
fn loop_file(project: &Path) -> Value {
    serde_json::from_str(&read_text(&project.join("control-loop.json"))).unwrap_or_default()
}

/// The identifier of the copy's loop (`control-loop.json`, `id`).
fn loop_id_of(project: &Path) -> String {
    loop_file(project)["id"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The configured simulated plant of the copy's loop (`control-loop.json`, `plantId`). It writes the loop's measured output and
/// reads its plant input at the loop's own addresses, so the scenarios run against it (B5.3).
fn plant_of(project: &Path) -> String {
    loop_file(project)["plantId"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The loop's working study as `study.json` holds it, or `Value::Null` when there is none.
fn study_of(project: &Path) -> Value {
    let file = storage::study_directory(project, &loop_id_of(project)).join("study.json");
    fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// The number that a JSON value holds, when it is finite. The engine writes `NaN` and infinity as `null`.
fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|float| float.is_finite())
}

/// The ids of the loop's declared validation scenarios (`verificationScenarios` of `control-loop.json`, B5.3).
fn declared_scenario_ids(loop_definition: &LoopDefinition) -> Vec<String> {
    loop_definition
        .verification_scenarios
        .as_ref()
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |scenarios| {
            scenarios
                .iter()
                .filter_map(|scenario| scenario["id"].as_str().map(str::to_string))
                .collect()
        })
}

/// The answer as a failure message shows it: its samples, points, predictions and records are long, so they are left out.
fn without_series(answer: &Value) -> String {
    let mut shortened = answer.clone();
    prune(&mut shortened);
    shortened.to_string().chars().take(4_000).collect()
}

/// Removes the raw series from every object of `value`.
fn prune(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in ["samples", "points", "predictions", "record"] {
                let _ = map.remove(key);
            }
            for item in map.values_mut() {
                prune(item);
            }
        }
        Value::Array(items) => {
            for item in items {
                prune(item);
            }
        }
        _ => {}
    }
}

/// Checks the candidate of a result (B4.1, N6.4): unapplied, one of the four structures, and finite gains, which are positive
/// for lesson 85's `direct` action. Returns the proportional and the integral gain when both are finite.
fn check_candidate(findings: &mut Findings, candidate: &Value) -> Option<(f64, f64)> {
    let [proportional, integral, derivative] =
        ["proportionalGain", "integralGain", "derivativeGain"]
            .map(|key| number(&candidate["parameters"][key]));
    findings.check(
        candidate["applied"] == false,
        "the candidate is unapplied (B4.1)",
    );
    findings.check(
        matches!(
            candidate["structure"].as_str(),
            Some("P" | "PI" | "PD" | "PID")
        ),
        "the candidate has one of the structures P, PI, PD and PID",
    );
    findings.check(
        proportional.is_some_and(|gain| gain > 0.0)
            && [integral, derivative]
                .iter()
                .all(|gain| gain.is_some_and(|value| value >= 0.0)),
        "every gain is finite, Kp is positive and Ki and Kd are positive or zero (N6.4)",
    );
    proportional.zip(integral)
}

/// The model check passed (B5.2, N3.5). The check's recording (`recordingId`) is the Identify section's validation recording, which
/// is not its estimation recording (N3.4); the study lists that recording and keeps its file (B10.1, B10.3); and the model is no
/// longer provisional. The check tested the model that Identify keeps (`modelRevision`), and it reports its error and its limit
/// (N3.2, B5.5).
fn check_model_check_passed(findings: &mut Findings, lab: &Lab, study: &Value, result: &Value) {
    let check = &result["modelCheck"];
    findings.check(
        check["status"] == "Pass",
        "the result came after a passing model check (B5.2)",
    );
    let identify = &study["sections"]["identify"];
    let validation = identify["validationRecordingId"].as_str();
    findings.check(
        identify["provisional"] == false
            && validation.is_some()
            && validation == check["recordingId"].as_str()
            && validation != identify["estimationRecordingId"].as_str(),
        "on a pass the Identify section names the check's recording as its validation recording, not its estimation recording (B5.2, N3.4)",
    );
    let revision = &identify["selectedModel"]["revision"];
    findings.check(
        revision.as_str().is_some_and(|text| !text.is_empty())
            && check["modelRevision"] == *revision,
        "the check tested the model that Identify keeps (N3.4)",
    );
    if let Some(id) = validation {
        let listed = study["recordings"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|item| item.as_str() == Some(id)));
        findings.check(
            listed,
            format!("the study lists the validation recording {id} (B10.1)"),
        );
        let file = storage::study_directory(&lab.scratch.project, &lab.loop_definition.id)
            .join("recordings")
            .join(format!("{id}.json"));
        findings.check(
            file.is_file(),
            format!(
                "the validation recording is kept in the study (B5.2, B10.3): {}",
                file.display()
            ),
        );
    }
    findings.check(
        number(&check["normalizedError"]).is_some() && number(&check["limit"]).is_some(),
        "the model check reports its error (normalizedError) and its limit (B5.2, N3.2)",
    );
}

/// Every scenario runs the current settings and the candidate, each as an isolated simulation's run on the configured plant
/// (B5.3, E8.3, A9). The target response and each declared scenario of the loop are among the scenarios (layer 1's campaign).
/// Each run has its evidence (`ST-executed`) and its status, and the two runs of a scenario are distinct runs.
fn check_scenarios(findings: &mut Findings, lab: &Lab, result: &Value) {
    let empty = Vec::new();
    let scenarios = result["scenarios"].as_array().unwrap_or(&empty);
    let ids: Vec<&str> = scenarios
        .iter()
        .filter_map(|scenario| scenario["id"].as_str())
        .collect();
    let declared = declared_scenario_ids(&lab.loop_definition);
    let mut expected = vec![TARGET_RESPONSE];
    expected.extend(declared.iter().map(String::as_str));
    findings.check(
        !scenarios.is_empty() && expected.iter().all(|id| ids.contains(id)),
        format!(
            "the validation runs the target response and each declared scenario {declared:?} (B5.3): it ran {ids:?}"
        ),
    );
    let configured = plant_of(&lab.scratch.project);
    for scenario in scenarios {
        let id = scenario["id"].as_str().unwrap_or("(no id)");
        findings.check(
            scenario["plant"]
                .as_str()
                .is_some_and(|plant| plant.contains(&configured)),
            format!("scenario {id} runs against the configured plant {configured} (B5.3)"),
        );
        let current = &scenario["current"];
        let candidate = &scenario["candidate"];
        findings.check(
            current["runId"].as_str().is_some_and(|run| !run.is_empty())
                && current["runId"] != candidate["runId"],
            format!("scenario {id} has a current run and a candidate run, distinct (B5.3)"),
        );
        for (which, run) in [("current", current), ("candidate", candidate)] {
            findings.check(
                run["evidence"] == "ST-executed"
                    && matches!(run["status"].as_str(), Some("Pass" | "Fail" | "Inconclusive")),
                format!(
                    "the {which} run of scenario {id} is an isolated simulation's run, with its status (B5.3, E8.3, A9): {run}"
                ),
            );
        }
    }
}

/// Every requirement of every run has its result: its status, its value (a Pass has a measured value; a Fail has one, or the
/// reason it has none, as a response that never settles has no arrival time; an Inconclusive one may have none), its limit,
/// its unit, its source and its reason (B5.4). The overall result is the candidate's (N8.5): Fail
/// when a candidate requirement or run fails, Pass when every one passes, and Inconclusive otherwise. Apply is available exactly
/// when that result passes (B5.4), and the verb exits with 0 exactly then (§0.1b).
fn check_requirements(findings: &mut Findings, result: &Value, status: ExitStatus) {
    let empty = Vec::new();
    let scenarios = result["scenarios"].as_array().unwrap_or(&empty);
    let non_empty = |value: &Value| value.as_str().is_some_and(|text| !text.is_empty());
    let mut candidate_words: Vec<&str> = Vec::new();
    for scenario in scenarios {
        for which in ["current", "candidate"] {
            let run = &scenario[which];
            let requirements = run["requirements"].as_array().unwrap_or(&empty);
            for requirement in requirements {
                let word = requirement["status"].as_str().unwrap_or_default();
                let measured = word == "Inconclusive"
                    || number(&requirement["value"]).is_some()
                    || (word == "Fail" && non_empty(&requirement["reason"]));
                let complete = matches!(word, "Pass" | "Fail" | "Inconclusive")
                    && measured
                    && number(&requirement["limit"]).is_some()
                    && non_empty(&requirement["unit"])
                    && non_empty(&requirement["source"]);
                findings.check(
                    complete,
                    format!(
                        "requirement {} of the {which} run of {} has its result with value, limit, unit and source (B5.4): {requirement}",
                        requirement["id"], scenario["id"]
                    ),
                );
            }
            if which == "candidate" {
                candidate_words.push(run["status"].as_str().unwrap_or_default());
                candidate_words.extend(
                    requirements
                        .iter()
                        .map(|requirement| requirement["status"].as_str().unwrap_or_default()),
                );
            }
        }
    }
    let overall = if candidate_words.contains(&"Fail") {
        "Fail"
    } else if !candidate_words.is_empty() && candidate_words.iter().all(|word| *word == "Pass") {
        "Pass"
    } else {
        "Inconclusive"
    };
    findings.check(
        result["status"] == overall,
        format!("the overall result is {overall}: the candidate's worst requirement or run result (N8.5)"),
    );
    let apply_available = result["applyAvailable"] == true;
    findings.check(
        result["applyAvailable"].is_boolean() && apply_available == (overall == "Pass"),
        "Apply is available exactly when the candidate passes every requirement (B5.4)",
    );
    findings.check(
        (status.code() == Some(0)) == apply_available,
        format!("the verb exits with 0 exactly when Apply is available (§0.1b): status {status}"),
    );
}

/// Every isolated run of the validation completed (E8.3, E8.4): no run ended before its grid, so no requirement is Inconclusive
/// for want of a run. A program that designs its own record during its first scan keeps the candidate that the runtime writes
/// again at the task boundary after that scan.
fn check_runs_completed(findings: &mut Findings, result: &Value) {
    let empty = Vec::new();
    for scenario in result["scenarios"].as_array().unwrap_or(&empty) {
        let id = scenario["id"].as_str().unwrap_or("(no id)");
        for which in ["current", "candidate"] {
            let unfinished: Vec<&str> = scenario[which]["requirements"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .filter_map(|requirement| requirement["reason"].as_str())
                .filter(|reason| reason.contains("before it completed"))
                .collect();
            findings.check(
                unfinished.is_empty(),
                format!("the {which} run of scenario {id} completed (E8.3): {unfinished:?}"),
            );
        }
    }
}

/// The study keeps the Validate section (decision 2): it holds the same overall result and Apply's availability, it is current for
/// the model that Identify keeps (A8.4), and the candidate that Tune keeps stays unapplied (B4.1, N8.9).
fn check_validate_section(findings: &mut Findings, study: &Value, result: &Value) {
    let section = &study["sections"]["validate"];
    let revision = &study["sections"]["identify"]["selectedModel"]["revision"];
    findings.check(
        section.is_object(),
        "the study keeps the Validate section (decision 2)",
    );
    findings.check(
        section["status"] == result["status"],
        "the Validate section holds the overall result",
    );
    findings.check(
        section["applyAvailable"].is_boolean()
            && section["applyAvailable"] == result["applyAvailable"],
        "the Validate section records whether Apply is available (B5.4)",
    );
    findings.check(
        revision.as_str().is_some_and(|text| !text.is_empty())
            && section["modelRevision"] == *revision,
        "the Validate section is current for the model that Identify keeps (A8.4)",
    );
    findings.check(
        study["sections"]["tune"]["candidate"]["applied"] == false,
        "the candidate stays unapplied (B4.1, N8.9)",
    );
}

/// A validation that passed the model check answered with its result (B5.2 to B5.4, decision 2).
fn check_validated(findings: &mut Findings, lab: &Lab, result: &Value, status: ExitStatus) {
    let study = study_of(&lab.scratch.project);
    check_model_check_passed(findings, lab, &study, result);
    check_scenarios(findings, lab, result);
    check_requirements(findings, result, status);
    check_validate_section(findings, &study, result);
}

/// The model check failed (B5.2, B5.5): Validate stops with the check's failure, whose cause is the model or the data and whose
/// next action is Identify, and whose text names the error and the limit. The study keeps the failed check and the model stays
/// provisional, which keeps Apply unavailable (N3.5).
fn check_failed_model_check(
    findings: &mut Findings,
    lab: &Lab,
    failure: &Value,
    status: ExitStatus,
) {
    findings.check(
        status.code() == Some(1),
        format!("a failed model check exits with 1 (§0.1b): {status}"),
    );
    findings.check(
        failure["cause"] == "model-or-data" && failure["nextAction"] == "identify",
        format!(
            "the failed check has the cause model or data and the next action Identify (B5.5): {failure}"
        ),
    );
    let text = format!(
        "{} {}",
        failure["message"].as_str().unwrap_or_default(),
        failure["detail"].as_str().unwrap_or_default()
    );
    findings.check(
        text.to_lowercase().contains("limit"),
        format!("the failure names the error and the limit (B5.5): {text}"),
    );
    let study = study_of(&lab.scratch.project);
    let identify = &study["sections"]["identify"];
    findings.check(
        identify["provisional"] == true && identify["validationRecordingId"].is_null(),
        "a failed check does not qualify the model (B5.2, N3.5)",
    );
    let model_check = &study["sections"]["validate"]["modelCheck"];
    findings.check(
        model_check["status"] == "Fail"
            && number(&model_check["normalizedError"]).is_some()
            && number(&model_check["limit"]).is_some(),
        "the study keeps the failed check with its error and its limit (decision 2)",
    );
}

/// `validate` before any model is refused at once, with Identify as its next action (decision 3, B5). No runtime is running in
/// this copy, so a validation that reached the runtime would fail with another cause. It stores nothing and creates no study.
///
/// Run time: well under a second; the copy of the example and one start of the program are the only work.
#[test]
fn validate_before_identify_is_refused_at_once_with_identify_as_the_next_action() {
    let scratch = Scratch::new("refused", LESSON_85);
    let journal = scratch.folder.join("journal");
    let arguments = verb_arguments("validate", &scratch.project, &journal, &["--json"]);
    let mut program = Program::start(&arguments, true, &scratch.folder, "validate");
    let status = program.wait(DEADLINE);
    let answer = program.answer();

    let mut findings = Findings {
        shown: without_series(&answer),
        ..Findings::default()
    };
    findings.check(
        status.code() == Some(1),
        format!(
            "validate without a model is refused with status 1:\n{}",
            program.report(status)
        ),
    );
    findings.check(
        answer["failure"]["nextAction"] == "identify" && answer.get("result").is_none(),
        "without a model the next action is Identify (B5, decision 3)",
    );
    findings.check(
        study_of(&scratch.project).is_null(),
        "a refused validation creates no study",
    );
    findings.conclude();
}

/// `validate --json` on the lesson `example`, after `identify` and `tune --method simc` (decisions 1 and 3; B5.1 to B5.5). The
/// SIMC candidate differs from the program's own gains, which is what makes a write to the live controller visible. The checks
/// are described in the module documentation.
fn validate_on(name: &str, example: &str) {
    let mut findings = Findings::default();
    let lab = Lab::start(name, example);
    let identify = lab.identify(&mut findings);
    findings.check(
        identify["provisional"] == true,
        "the model is provisional after identify, so the model check runs (B5.2)",
    );
    lab.check_refused(&mut findings, "tune");
    let tuned = lab.tune(
        &mut findings,
        &["--method", "simc", "--endpoint", UNREACHABLE],
    );
    let candidate_kp = check_candidate(&mut findings, &tuned["candidate"]).map(|(kp, _)| kp);
    let live_before = lab.live_settings();
    let live_kp = live_before.first().copied().flatten();
    findings.check(
        matches!((candidate_kp, live_kp), (Some(designed), Some(live)) if (designed - live).abs() > GAIN_DIFFERENCE),
        "the candidate's Kp differs from the live Kp, so a write would show (B5.3)",
    );

    let (program, status) = lab.run("validate", &["--json"], true, VALIDATE_LIMIT);
    let answer = program.answer();
    findings.shown = without_series(&answer);
    let live_after = lab.live_settings();
    findings.check(
        live_before == live_after,
        format!(
            "validate writes nothing to the live controller (N8.9): {live_before:?} before, {live_after:?} after"
        ),
    );
    findings.check(
        matches!(status.code(), Some(0 | 1)),
        format!(
            "validate ends with status 0 or 1 (§0.1b):\n{}",
            program.report(status)
        ),
    );
    match (answer.get("result"), answer.get("failure")) {
        (Some(result), None) => {
            check_runs_completed(&mut findings, result);
            check_validated(&mut findings, &lab, result, status);
        }
        (None, Some(failure)) => check_failed_model_check(&mut findings, &lab, failure, status),
        _ => findings.check(
            false,
            format!(
                "validate answers with one result or one failure:\n{}",
                program.report(status)
            ),
        ),
    }
    findings.conclude();
}

/// Lesson 85: identify, tune and validate.
///
/// Run time at ten times real time: about 10 s, nearly all of it the build, the start of the runtime, `identify` and the model
/// check's live campaign; the isolated runs do not follow the live speed (A10.5).
#[test]
fn validate_runs_the_model_check_and_the_isolated_scenarios_on_lesson_85() {
    validate_on("validate-85", LESSON_85);
}

/// Lesson 30, whose program designs its own startup gains during its first scan: every isolated run completes, the candidate's
/// with the candidate written again at the task boundary after that scan (E8.3), instead of failing as a candidate the program
/// does not adopt.
///
/// Run time at ten times real time: about 15 s; lesson 30 also runs its two declared scenarios.
#[test]
fn validate_runs_the_model_check_and_the_isolated_scenarios_on_lesson_30() {
    validate_on("validate-30", LESSON_30);
}
