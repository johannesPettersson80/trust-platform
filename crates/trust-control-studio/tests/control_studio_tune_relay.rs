//! Control Studio's relay rule through the built program `trust-control-studio` on a real runtime (stage V2, layer 4a;
//! specification 35, §0.1b and B4.1 to B4.2; numerical annex N2.9, N5.R and N6.4).
//!
//! The program runs on a copy of lesson 29, `examples/control/06-pid-tuning-methods/29-relay-tuning` (`EXAMPLE` and `LOOP_ID`
//! are the only places that name it), against `trust-runtime run --simulation` at ten times real time (`time_scale = 10` in the
//! copy's `simulation.toml`, the speed the other runtime tests set). The copy's endpoint is a socket in the copy, and the control
//! token goes to `identify` through `TRUST_CONTROL_TOKEN` (§0.1b). The copy's `src/tests.st` declares the lesson's
//! `TEST_PROGRAM`s, which an ordinary runtime build rejects, so the copy takes them out, as `identify_lab` does. Each test works on
//! a copy of its own, so the tests run side by side.
//!
//! The loop's controller acts `direct` (`control-loop.json`), so the gains of a relay candidate are positive (N6.4). The rule gives
//! `Kp = 0.45·Ku` and `Ti = Pu/1.2`, with `Ki = Kp/Ti` (N5.R).
//!
//! The lesson's README states the cycle of its own run: `d` = 10 %, `a` = 0.4354 °C, `Pu` = 4.42 s, `Ku` = 29.24 %/°C, `Kp` =
//! 13.16 %/°C and `Ki` = 3.57 %/(°C·s). The test identifies the heater again and measures its own cycles, so the README's numbers
//! are compared with a tolerance of 15 %, as `control_studio_tune.rs` compares its SIMC gains with the README's: the record is
//! noisy, and the lesson's own repeated relay test gave `Kp` within 4 % of the first. The formulas are checked against the `Ku`
//! and `Pu` that the reason prints, to 1 %, because the reason prints them with two decimals.
//!
//! Test 1 identifies with the relay and designs `tune --method relay`: the candidate is an unapplied PI of the relay rule whose
//! formulas and the README agree. Test 2 identifies the same copy with a plant input step, so the method is the only difference,
//! and checks that the relay rule is refused with the next action Choose method: it is offered only for a model fitted from a
//! qualified Feedback relay record of the same study (N5.R, B4.7).
//!
//! # Running
//!
//! `cargo test -p trust-control-studio --test control_studio_tune_relay`. The tests that start a runtime run the `trust-runtime`
//! executable of their own target folder, so build it first with `cargo build -p trust-runtime --bin trust-runtime`, or name
//! another one in `TRUST_RUNTIME_BIN`. The file is Unix only, as the other runtime tests are: the runtime's endpoint is a Unix
//! socket.
#![cfg(unix)]

mod common;

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::Value;
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::loop_inspection_bindings;
use trust_control_studio::runtime_client::RuntimeClient;
use trust_control_studio::{loops, storage};

use common::{
    copy_dir, inspect_until_available, prepare_runtime_toml, run_trust_runtime, start_runtime,
    wait_for_status, RemoveDir, Runtime, DEADLINE, TOKEN,
};

/// The relay heater: lesson 29 (module documentation).
const EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/29-relay-tuning"
);
/// The identifier of lesson 29's loop (`control-loop.json`).
const LOOP_ID: &str = "control-relay-tuning";
/// The runtime the verbs name for a project's loops; the example's loop is bound to it.
const RUNTIME_ID: &str = "simulator";
/// The speed of the simulation, ten times real time.
const TIME_SCALE: u32 = 10;
/// How often a test looks at a running program.
const POLL: Duration = Duration::from_millis(25);
/// The longest `identify` may run; at ten times real time it takes seconds, so this only ends a hung run.
const RUN_LIMIT: Duration = Duration::from_secs(180);
/// How far the formulas may be from the printed `Ku` and `Pu`: the reason prints them with two decimals, and the engine keeps
/// eight significant digits of the gains.
const FORMULA_TOLERANCE: f64 = 0.01;
/// The README's relay cycle and gains (module documentation): `Ku` in %/°C, `Pu` in s, and `Kp` and `Ki` of the PI.
const LESSON_KU: f64 = 29.24;
const LESSON_PU: f64 = 4.42;
const LESSON_KP: f64 = 13.16;
const LESSON_KI: f64 = 3.57;
/// How far the measured numbers may be from the README's (module documentation).
const LESSON_TOLERANCE: f64 = 0.15;
/// The refusal of the relay rule without a relay record (N5.R; the sentence of `relayCycleEvidence.ts`).
const NEEDS_RELAY_RECORD: &str =
    "Identify the plant with Feedback relay and repeatable cycles before using the relay rule.";

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

/// A folder of one test with a copy of the example in it, without its test programs. The folder is removed when the test ends,
/// even when it fails.
struct Scratch {
    folder: PathBuf,
    project: PathBuf,
    _remove: RemoveDir,
}

impl Scratch {
    /// A folder named after `name`, holding a copy of lesson 29 whose test programs are taken out.
    fn new(name: &str) -> Self {
        let folder =
            std::env::temp_dir().join(format!("trust-cs-tune-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let remove = RemoveDir(folder.clone());
        let project = folder.join("project");
        copy_dir(Path::new(EXAMPLE), &project);
        remove_test_programs(&project);
        Self {
            folder,
            project,
            _remove: remove,
        }
    }
}

/// Removes the copied sources that declare a `TEST_PROGRAM`. The lesson's ST unit tests run under `trust-dev test`, which
/// registers its test programs; an ordinary runtime build rejects an unconfigured test program (specification 22, test
/// discovery), as `identify_lab` also does.
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

/// A copy of lesson 29 with its runtime running at ten times real time, and the folder of the operation journals.
struct Lab {
    /// The runtime process. It is declared first, so it ends before the folder goes.
    _runtime: Runtime,
    scratch: Scratch,
    /// The folder of the operation journals, passed as `--journal-dir`.
    journal: PathBuf,
}

impl Lab {
    /// Copies lesson 29, sets its speed, builds it, starts the runtime and waits for the first scan.
    fn start(name: &str) -> Self {
        let scratch = Scratch::new(name);
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
        let loop_definition = loops::open(project, RUNTIME_ID, LOOP_ID)
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
        }
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

    /// Identifies the heater with `identify --method <method> --json`, with the control token (§0.1b). Nothing after it can run
    /// without the model it keeps, so when the verb does not finish with its model the test ends here with the findings so far.
    fn identify(&self, findings: &mut Findings, method: &str) {
        let (program, status) =
            self.run("identify", &["--method", method, "--json"], true, RUN_LIMIT);
        let kept = study_of(&self.scratch.project)["sections"]["identify"].is_object();
        let finished = status.code() == Some(0) && kept;
        findings.check(
            finished,
            format!(
                "identify --method {method} should finish with its result and keep its model:\n{}",
                program.report(status)
            ),
        );
        if !finished {
            findings.conclude();
        }
    }

    /// Runs `tune --json` with `extra` arguments and without the control token: Tune never reaches the runtime (B4.1). Returns the
    /// `result` of its answer; the test ends here with the findings so far when the verb does not finish with one.
    fn tune(&self, findings: &mut Findings, extra: &[&str]) -> Value {
        let mut arguments = vec!["--json"];
        arguments.extend_from_slice(extra);
        let (program, status) = self.run("tune", &arguments, false, DEADLINE);
        let result = serde_json::from_str::<Value>(&read_text(&program.out))
            .ok()
            .and_then(|answer| answer.get("result").cloned());
        findings.check(
            status.code() == Some(0) && result.is_some(),
            format!(
                "tune should finish with its result (status 0):\n{}",
                program.report(status)
            ),
        );
        // The predicted response is long, and the checks of it read its length only.
        let mut shown = result.clone().unwrap_or_default();
        if let Some(object) = shown.as_object_mut() {
            object.remove("response");
        }
        findings.shown = shown.to_string();
        if result.is_none() {
            findings.conclude();
        }
        result.unwrap_or_default()
    }
}

/// `tune --method relay` after a relay identification gives the relay rule's PI, unapplied (N5.R, B4.1, B4.2). Lesson 29's loop is
/// in Manual at 75 % and permits the relay (its `relayDirection` is declared), so `identify --method relay --json` measures the
/// cycles; then `tune --method relay --json` designs from them without the control token. The test checks the result's method
/// (relay), the candidate's source (`Relay rule`) and structure (PI), the gain signs of the `direct` action (Kp and Ki positive, no
/// derivative term, N6.4), the method description (it names Ku and Pu), the formulas against the Ku and Pu of the reason (1 %), the
/// README's numbers (15 %), the Tune stage, and the study's Tune section, which keeps the candidate unapplied and names the model it
/// was designed on (A8.4).
///
/// Run time at ten times real time: about 10 to 15 s: the copy and the build, the start of the runtime, the relay's 6 s noise hold
/// and at most 60 s of switching (6.6 s of wall time at most), and a tune that reads the stored recording in well under a second.
#[test]
fn relay_tune_gives_the_relay_rule_pi_of_the_measured_cycle() {
    let mut findings = Findings::default();
    let lab = Lab::start("relay-rule");
    lab.identify(&mut findings, "relay");
    let result = lab.tune(&mut findings, &["--method", "relay"]);

    findings.check(
        result["method"] == "relay",
        format!("the result names the relay method: {}", result["method"]),
    );
    let gains = check_candidate(&mut findings, &result["candidate"]);
    let description = result["methodDescription"].as_str().unwrap_or_default();
    findings.check(
        description.contains("Ku") && description.contains("Pu"),
        format!("the method description names Ku and Pu: {description:?}"),
    );
    if let Some(pair) = gains {
        check_formulas(
            &mut findings,
            result["reason"].as_str().unwrap_or_default(),
            pair,
        );
    }
    findings.check(
        stage_status(&result, "Tune") == Some("complete"),
        "the Tune stage is complete after the result",
    );
    check_stored_tune(&mut findings, &study_of(&lab.scratch.project));
    findings.conclude();
}

/// `tune --method relay` on a model that a plant input step fitted is refused (N5.R, B4.7): status 1, with the cause the model or the
/// data, the next action Choose method, the sentence that the relay rule needs a relay record, and no Tune section stored. The copy
/// of lesson 29 is identified with `identify --method step --json` (the loop permits the open-loop methods, B3.3), so the method is
/// the only difference from the first test.
///
/// Run time at ten times real time: about 8 to 12 s: the copy and the build, the start of the runtime, the step's hold of about 2 s
/// of wall time (as `control_studio_identify_methods.rs` measures a step on lesson 30), and a refusal at once.
#[test]
fn relay_rule_is_refused_for_a_model_identified_with_a_plant_input_step() {
    let mut findings = Findings::default();
    let lab = Lab::start("step-refusal");
    lab.identify(&mut findings, "step");
    let (program, status) = lab.run("tune", &["--json", "--method", "relay"], false, DEADLINE);
    let answer = serde_json::from_str::<Value>(&read_text(&program.out)).unwrap_or_default();
    findings.shown = answer.to_string();
    let failure = &answer["failure"];

    findings.check(
        status.code() == Some(1),
        format!(
            "tune --method relay on a step model should be refused with status 1:\n{}",
            program.report(status)
        ),
    );
    findings.check(
        failure["nextAction"] == "choose-method" && failure["cause"] == "model-or-data",
        format!(
            "the refusal's next action is Choose method, and its cause the model or the data (N5.R, B4.7): {failure}"
        ),
    );
    findings.check(
        failure["message"] == NEEDS_RELAY_RECORD,
        format!("the refusal says what the relay rule needs (N5.R): {failure}"),
    );
    findings.check(
        study_of(&lab.scratch.project)["sections"]
            .get("tune")
            .is_none(),
        "a refused tune stores no tune section (B4.7)",
    );
    findings.conclude();
}

/// Checks the candidate of a relay result (B4.1, N5.R, N6.4): unapplied, the relay rule's source, a PI with no derivative term, and
/// positive `Kp` and `Ki`, the signs of the `direct` action. Returns `Kp` and `Ki` when both are finite numbers.
fn check_candidate(findings: &mut Findings, candidate: &Value) -> Option<(f64, f64)> {
    let proportional = number(&candidate["parameters"]["proportionalGain"]);
    let integral = number(&candidate["parameters"]["integralGain"]);
    let derivative = number(&candidate["parameters"]["derivativeGain"]);
    findings.check(
        candidate["applied"] == false,
        "the candidate is unapplied (B4.1)",
    );
    findings.check(
        candidate["source"] == "Relay rule",
        format!(
            "the candidate's source is the relay rule (N5.R): {}",
            candidate["source"]
        ),
    );
    findings.check(
        candidate["structure"] == "PI",
        format!(
            "the relay rule gives a PI (N5.R): {}",
            candidate["structure"]
        ),
    );
    findings.check(
        derivative.is_some_and(|gain| gain.abs() <= 1e-12),
        format!("a PI has no derivative gain (N5.R): {derivative:?}"),
    );
    findings.check(
        proportional.is_some_and(|gain| gain > 0.0) && integral.is_some_and(|gain| gain > 0.0),
        format!(
            "Kp and Ki are positive, the signs of the direct action (N6.4): {proportional:?} and {integral:?}"
        ),
    );
    proportional.zip(integral)
}

/// Checks the relay rule's formulas (N5.R, B4.2) against the `Ku` and `Pu` that the reason prints, and both against the lesson's
/// README (module documentation): `Kp = 0.45·Ku` and `Ti = Pu/1.2` with `Ki = Kp/Ti` hold within 1 %, and the README's `Ku`, `Pu`,
/// `Kp` and `Ki` are within 15 %.
fn check_formulas(findings: &mut Findings, reason: &str, (proportional, integral): (f64, f64)) {
    let printed_gain = number_after(reason, "Ku = ");
    let printed_period = number_after(reason, "Pu = ");
    findings.check(
        printed_gain.is_some() && printed_period.is_some(),
        format!("the reason prints Ku and Pu (N5.R): {reason:?}"),
    );
    let (Some(ultimate_gain), Some(ultimate_period)) = (printed_gain, printed_period) else {
        return;
    };
    findings.check(
        near(Some(proportional), 0.45 * ultimate_gain, FORMULA_TOLERANCE),
        format!("Kp is 0.45 Ku, with Ku {ultimate_gain}: Kp is {proportional}"),
    );
    let integral_time = proportional / integral;
    findings.check(
        near(
            Some(integral_time),
            ultimate_period / 1.2,
            FORMULA_TOLERANCE,
        ),
        format!("Ti is Pu / 1.2, with Pu {ultimate_period} s: Ti is {integral_time} s"),
    );
    for (name, actual, expected) in [
        ("Ku", ultimate_gain, LESSON_KU),
        ("Pu", ultimate_period, LESSON_PU),
        ("Kp", proportional, LESSON_KP),
        ("Ki", integral, LESSON_KI),
    ] {
        findings.check(
            near(Some(actual), expected, LESSON_TOLERANCE),
            format!("{name} is within 15 % of the README's {expected}: it is {actual}"),
        );
    }
}

/// Checks the study on disk after the relay tune (A8.4, B4.1): it keeps its Identify section, and its Tune section holds the
/// unapplied relay candidate, named with the revision of the model it was designed on.
fn check_stored_tune(findings: &mut Findings, study: &Value) {
    let sections = &study["sections"];
    let tune = &sections["tune"];
    let revision = &sections["identify"]["selectedModel"]["revision"];
    findings.check(
        sections["identify"].is_object(),
        "the study keeps its identify section (B4.1)",
    );
    findings.check(
        tune["method"] == "relay" && tune["candidate"]["applied"] == false,
        format!("the study's Tune section holds the unapplied relay candidate (B4.1): {tune}"),
    );
    findings.check(
        revision.as_str().is_some_and(|text| !text.is_empty())
            && tune["modelRevision"] == *revision,
        format!(
            "the Tune section names the revision {revision} of the model it was designed on (A8.4)"
        ),
    );
}

/// The number that follows `label` in `text`, up to the next space: the reason of a relay result prints `Ku = {Ku} and Pu = {Pu} s`.
fn number_after(text: &str, label: &str) -> Option<f64> {
    text.split_once(label)?
        .1
        .split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()
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
    arguments.extend(["--loop", LOOP_ID, "--journal-dir"].map(String::from));
    arguments.push(journal.display().to_string());
    arguments.extend(extra.iter().map(|argument| (*argument).to_string()));
    arguments
}

/// The loop's working study as `study.json` holds it, or `Value::Null` when there is none.
fn study_of(project: &Path) -> Value {
    let file = storage::study_directory(project, LOOP_ID).join("study.json");
    fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// The number that a JSON value holds, when it is finite. The engine writes `NaN` and infinity as `null`.
fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|float| float.is_finite())
}

/// Whether `actual` is within `relative` of `expected`, with a slack of `1e-12` for an expected zero.
fn near(actual: Option<f64>, expected: f64, relative: f64) -> bool {
    actual.is_some_and(|value| (value - expected).abs() <= relative * expected.abs() + 1e-12)
}

/// The status word of `stage` in the stages of an answer (§4.2), for example `complete` or `needs-data`.
fn stage_status<'a>(answer: &'a Value, stage: &str) -> Option<&'a str> {
    answer["stages"]
        .as_array()?
        .iter()
        .find(|entry| entry["stage"] == stage)
        .and_then(|entry| entry["status"].as_str())
}
