//! Control Studio's `tune` verb through the built program `trust-control-studio`, on the model that `identify` leaves in the
//! study of a real runtime (stage V2, layer 3; specification 35, §0.1b, A8.4 and B4.1 to B4.7; numerical annex N5.2, N5.4,
//! N5.10, N6.4 and N7.7).
//!
//! The program runs on a copy of lesson 30's heater project, `examples/control/06-pid-tuning-methods/30-model-based-pi-tuning`
//! (`EXAMPLE` and `LOOP_ID` are the only places that name it), against `trust-runtime run --simulation` at ten times real time
//! (`time_scale = 10` in the copy's `simulation.toml`, the speed the user chose for lessons, as `control_studio_identify.rs`
//! sets it). The runtime's endpoint comes from the copy's `runtime.toml`, which the test points at a socket in the copy; the
//! control token goes to `identify` through `TRUST_CONTROL_TOKEN` (§0.1b), and every run names its own `--journal-dir`. Each
//! test works on a copy of its own, so the tests run side by side. The loop's controller acts `direct`, so the gains of a
//! candidate are positive (N6.4). The lesson's README states that Control Studio's SIMC designs `Kp` 3.21 and `Ki` 0.655 for
//! the model it identifies, `K` 0.2 degC per %, `T` 4.9 s and `L` 1.1 s, at `λ` = 6.5 s: More robust, `4·max(L, T/3)` (N5.4).
//!
//! The tests prove that `tune` before `identify` is refused with the next action Identify and stores nothing (B4.7); that `tune`
//! needs no runtime (B4.1, B4.5: a runtime it cannot read only leaves out the current settings' prediction), since it finishes
//! with its result although its `--endpoint` is one that nothing listens on and it has no control token, with an unapplied
//! candidate of the loop's gain sign (N6.4) and finite margins (B4.6), stored in
//! a Tune section designed on the study's model (A8.4); and that `tune --method simc` gives the gains that N5.2 and N5.4 give
//! for the identified model's own parameters, close to the README's. A failed check is recorded as a finding and the test goes
//! on; a step that needs the result of the last ends the test with the findings so far and the JSON they read.
//!
//! # Running
//!
//! `cargo test -p trust-control-studio --test control_studio_tune`. The tests that start a runtime run the `trust-runtime`
//! executable of their own target folder, so build it first with `cargo build -p trust-runtime --bin trust-runtime`, or name
//! another one in `TRUST_RUNTIME_BIN` (`cargo test --all` builds it). The file is Unix only, as `control_studio_identify.rs`
//! is: the runtime's endpoint is a Unix socket.
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

/// The heater project the tests copy (see the module documentation).
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
/// A control endpoint that nothing listens on: a run that tried to reach it would be refused at once.
const UNREACHABLE: &str = "tcp://127.0.0.1:1";
/// How often a test looks at a running program.
const POLL: Duration = Duration::from_millis(25);
/// The longest `identify` may run; at ten times real time it takes seconds, so this only ends a hung run.
const RUN_LIMIT: Duration = Duration::from_secs(180);
/// The relative agreement of a stored gain with its formula: the engine keeps eight significant digits of `Kp` and `Ti`.
const FORMULA_TOLERANCE: f64 = 1e-6;
/// The README's SIMC `(Kp, Ki)` for its model (module documentation).
const LESSON_GAINS: (f64, f64) = (3.21, 0.655);
/// How far the identified gains may be from the README's: the uncertainty of the identification, since the README's model is
/// another identification of the same heater and `Kp` follows `T / (K (λ + L))`.
const LESSON_TOLERANCE: f64 = 0.15;
/// The label of every candidate of the constrained comparison except the formula's own (N5.10).
const CONSTRAINED: &str = "Constrained comparison";

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
    fn new(name: &str) -> Self {
        let folder =
            std::env::temp_dir().join(format!("trust-cs-tune-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let remove = RemoveDir(folder.clone());
        let project = folder.join("project");
        copy_dir(Path::new(EXAMPLE), &project);
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
        let mut command = Command::new(
            std::env::var_os("CARGO_BIN_EXE_trust-control-studio").expect(
                "Cargo must provide trust-control-studio binary while executing integration tests",
            ),
        );
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

/// A copy of the example with its runtime running at ten times real time, and the folder of the journals.
struct Lab {
    /// The runtime process. It is declared first, so it ends before the folder goes.
    _runtime: Runtime,
    scratch: Scratch,
    /// The folder of the operation journals, passed as `--journal-dir`.
    journal: PathBuf,
}

impl Lab {
    /// Copies lesson 30, sets its speed, builds it, starts the runtime and waits for the first scan.
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
            runtime_id: None,
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

    /// Identifies the heater with `identify --json`, the loop's own method (Automatic: a plant-input step in Manual, B3.3), and
    /// returns the Identify section that the study keeps. Nothing after it can run without the model, so when the verb does not
    /// finish with its result the test ends here with the findings so far.
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

    /// Runs `tune --json` with `extra` arguments and without the control token: Tune needs no runtime (B4.1, B4.5). Returns
    /// the `result` of its answer; the test ends here with the findings so far when the verb does not finish with one.
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

/// Whether both numbers of `actual` are within `relative` of those of `expected`.
fn near_pair(actual: Option<(f64, f64)>, expected: (f64, f64), relative: f64) -> bool {
    actual.is_some_and(|pair| {
        near(Some(pair.0), expected.0, relative) && near(Some(pair.1), expected.1, relative)
    })
}

/// The status word of `stage` in the stages of an answer (§4.2), for example `complete` or `needs-data`.
fn stage_status<'a>(answer: &'a Value, stage: &str) -> Option<&'a str> {
    answer["stages"]
        .as_array()?
        .iter()
        .find(|entry| entry["stage"] == stage)
        .and_then(|entry| entry["status"].as_str())
}

/// The gain, time constant and dead time of the model in an Identify section, from the parameters of its presentation (B3.8).
/// `None` unless the model has exactly one time constant.
fn identified_model(identify: &Value) -> Option<(f64, f64, f64)> {
    let parameter = |name: &str| -> Option<f64> {
        let found = identify["presentation"]["parameters"]
            .as_array()?
            .iter()
            .find(|item| item["name"] == name)?;
        number(&found["value"])
    };
    if parameter("Time constant 2").is_some() {
        return None;
    }
    Some((
        parameter("Gain")?,
        parameter("Time constant 1")?,
        parameter("Dead time")?,
    ))
}

/// The SIMC PI of N5.2 for `K·exp(−L·s) / (T·s + 1)` at `λ = factor·max(L, T/3)` (N5.4): `Kp = T / (|K|·(λ + L))`,
/// `Ti = min(T, 4·(λ + L))`, `Ki = Kp / Ti`. Returns `λ` and the magnitudes of `Kp` and `Ki`.
fn simc_pi(gain: f64, time_constant: f64, dead_time: f64, factor: f64) -> (f64, f64, f64) {
    let lambda = factor * dead_time.max(time_constant / 3.0);
    let horizon = lambda + dead_time;
    let integral_time = time_constant.min(4.0 * horizon);
    let proportional = time_constant / (gain.abs() * horizon);
    (lambda, proportional, proportional / integral_time)
}

/// Checks the candidate of a result (B4.1, C1, N6.4): unapplied, one of the four structures, and finite gains, which are
/// positive for lesson 30's `direct` action. Returns the proportional and the integral gain when both are finite.
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
        "every gain is finite, Kp is positive and Ki and Kd are positive or zero: the loop acts direct (N6.4)",
    );
    proportional.zip(integral)
}

/// Checks the margins of a result (B4.6, N7.7). A loop with an integral term and a dead time has a gain crossover (the
/// integrator) and a phase crossover (the delay), so its margins are finite numbers, `{"finite": …}`.
fn check_margins(findings: &mut Findings, result: &Value) {
    let margins = &result["margins"];
    let integral = matches!(
        result["candidate"]["structure"].as_str(),
        Some("PI" | "PID")
    );
    let finite = |key: &str| number(&margins[key]["finite"]).is_some();
    findings.check(
        !integral || (finite("gainMarginDb") && finite("phaseMarginDegrees")),
        "a loop with an integral term has finite gain and phase margins (N7.7)",
    );
    findings.check(
        number(&margins["peakSensitivity"]).is_some(),
        "the margins give a finite peak sensitivity (B4.6)",
    );
}

/// Checks what an Automatic result shows besides its candidate: no comparison (N5.10), the generated requirements listed as
/// Automatic (B4.3), a predicted response of at least two points (B4.5), and the Identify and Tune stages complete.
fn check_automatic(findings: &mut Findings, result: &Value) {
    let listed = |value: &Value, wanted: usize| value.as_array().map(Vec::len) >= Some(wanted);
    findings.check(
        result["comparison"].as_array().is_some_and(Vec::is_empty),
        "Automatic gives no constrained comparison (N5.10)",
    );
    findings.check(
        listed(&result["requirements"]["automaticCriteria"]["fields"], 1),
        "the generated requirements are listed as Automatic (B4.3)",
    );
    findings.check(
        listed(&result["response"]["timeSeconds"], 2),
        "the result has a predicted response (B4.5)",
    );
    for stage in ["Identify", "Tune"] {
        findings.check(
            stage_status(result, stage) == Some("complete"),
            format!("the {stage} stage is complete after the result"),
        );
    }
}

/// Checks the study on disk after a tune (A8.1, A8.4, B4.1): it keeps the Identify section, and its Tune section holds an
/// unapplied candidate designed on the model of the Identify section. Returns the Tune section.
fn check_stored_tune(findings: &mut Findings, study: &Value) -> Value {
    let sections = &study["sections"];
    let tune = sections["tune"].clone();
    let revision = &sections["identify"]["selectedModel"]["revision"];
    findings.check(
        sections["identify"].is_object() && tune["candidate"]["applied"] == false,
        format!(
            "the study keeps its identify section and holds an unapplied candidate (B4.1): {tune}"
        ),
    );
    findings.check(
        revision.as_str().is_some_and(|text| !text.is_empty())
            && tune["modelRevision"] == *revision,
        format!(
            "the Tune section names the revision {revision} of the model it was designed on (A8.4)"
        ),
    );
    tune
}

/// Checks the constrained comparison of a SIMC result (N5.10): five candidates, the formula PI labelled SIMC first with the
/// candidate's gains `(Kp, Ki)`, and four labelled `Constrained comparison`.
fn check_comparison(findings: &mut Findings, comparison: &Value, formula: (f64, f64)) {
    let empty = Vec::new();
    let entries = comparison.as_array().unwrap_or(&empty);
    let labels: Vec<&str> = entries
        .iter()
        .filter_map(|entry| entry["label"].as_str())
        .collect();
    findings.check(
        labels == ["SIMC", CONSTRAINED, CONSTRAINED, CONSTRAINED, CONSTRAINED],
        "the comparison holds the formula PI labelled SIMC and four Constrained comparison candidates (N5.10)",
    );
    let first = entries.first().map(|entry| &entry["gains"]);
    let gains = first
        .and_then(|found| number(&found["proportionalGain"]).zip(number(&found["integralGain"])));
    findings.check(
        near_pair(gains, formula, FORMULA_TOLERANCE),
        "the first candidate of the comparison has the gains of the SIMC candidate",
    );
}

/// `tune` before `identify` is refused: the study has no model, so the verb finishes with status 1 and, with `--json`,
/// `{"failure": …}` on standard output whose cause is the model or the data and whose next action is Identify (B4.7, §0.1b).
/// Nothing is stored: the study has no Tune section. No runtime runs, and the program has no control token.
///
/// Run time: well under a second; the copy of the example and one start of the program are the only work (no runtime is built
/// or started, so the time scale does not apply).
#[test]
fn tune_before_identify_is_refused_with_identify_as_the_next_action() {
    let scratch = Scratch::new("refused");
    let journal = scratch.folder.join("journal");
    let arguments = verb_arguments("tune", &scratch.project, &journal, &["--json"]);
    let mut program = Program::start(&arguments, false, &scratch.folder, "tune");
    let status = program.wait(DEADLINE);

    let mut findings = Findings::default();
    let answer = serde_json::from_str::<Value>(&read_text(&program.out)).unwrap_or_default();
    findings.shown = answer.to_string();
    let failure = &answer["failure"];
    findings.check(
        status.code() == Some(1),
        format!(
            "tune without a model is refused with status 1:\n{}",
            program.report(status)
        ),
    );
    findings.check(
        failure["cause"] == "model-or-data" && failure["nextAction"] == "identify",
        "the cause is the model or the data, and the next action is Identify",
    );
    findings.check(
        study_of(&scratch.project)["sections"].get("tune").is_none(),
        "a refused tune stores no tune section",
    );
    findings.conclude();
}

/// `tune` needs no runtime and designs an unapplied candidate (B4.1, B4.3 to B4.6, A8.4). The heater is identified
/// through the program, then `tune --json` runs with `--endpoint tcp://127.0.0.1:1`, which nothing listens on, and without a
/// control token: it finishes with its result only if it never connects.
///
/// Run time at ten times real time: the build of the project and the start of the runtime take a few seconds. `identify` holds
/// the step for at most 57 s of process time and finishes early once the response settles, about 21 s (see
/// `control_studio_identify.rs`), so 2.4 s to 6 s of wall time plus about a second to keep and fit the recording; `tune` designs
/// on the stored model in well under a second. The test takes about 12 to 25 s.
#[test]
fn tune_never_reaches_the_runtime_and_designs_an_unapplied_pi_candidate() {
    let mut findings = Findings::default();
    let lab = Lab::start("designs");
    lab.identify(&mut findings);
    let result = lab.tune(&mut findings, &["--endpoint", UNREACHABLE]);
    check_candidate(&mut findings, &result["candidate"]);
    check_margins(&mut findings, &result);
    check_automatic(&mut findings, &result);
    check_stored_tune(&mut findings, &study_of(&lab.scratch.project));
    findings.conclude();
}

/// `tune --method simc` gives the gains that the formula gives for the identified model (N5.2, N5.4, N6.4). The heater is
/// identified through the program; its gain `K`, time constant `T` and dead time `L` are read from the study's Identify section,
/// and `Kp = T / (K (λ + L))`, `Ti = min(T, 4 (λ + L))` and `Ki = Kp / Ti` are computed here with `λ = 4·max(L, T/3)`, the
/// response that the study's requirements declare when `--choice` is not given: More robust, as Automatic fills it. The
/// candidate's gains must equal them within `1e-6` (the engine stores eight significant digits), and lie within 15 % of the
/// README's `Kp` 3.21 and `Ki` 0.655, which belong to another identification of the same heater.
///
/// Run time at ten times real time: as for `tune_never_reaches_the_runtime_and_designs_an_unapplied_pi_candidate`, about 12 to
/// 25 s, nearly all of it the build, the start of the runtime and `identify`.
#[test]
fn simc_tune_matches_the_numbers_of_the_lesson_for_the_identified_model() {
    let mut findings = Findings::default();
    let lab = Lab::start("simc");
    let model = identified_model(&lab.identify(&mut findings));
    findings.check(
        model.is_some(),
        "the identified model is first order, with one time constant",
    );
    let result = lab.tune(&mut findings, &["--method", "simc"]);
    let Some((plant_gain, time_constant, dead_time)) = model else {
        findings.conclude();
        return;
    };

    let candidate = &result["candidate"];
    let designed = check_candidate(&mut findings, candidate);
    findings.check(
        result["method"] == "SIMC"
            && candidate["source"] == "SIMC"
            && candidate["structure"] == "PI",
        "a request for SIMC gives a SIMC PI",
    );
    let (lambda, kp, ki) = simc_pi(plant_gain, time_constant, dead_time, 4.0);
    let reported = number(&result["closedLoopTimeSeconds"]);
    findings.check(
        result["objective"] == "robust" && near(reported, lambda, FORMULA_TOLERANCE),
        format!("without --choice the response is More robust, with λ = {lambda} s (N5.4)"),
    );
    findings.check(
        near_pair(designed, (kp, ki), FORMULA_TOLERANCE),
        format!("(Kp, Ki) {designed:?} is ({kp}, {ki}) for K {plant_gain}, T {time_constant} s and L {dead_time} s (N5.2)"),
    );
    findings.check(
        near_pair(designed, LESSON_GAINS, LESSON_TOLERANCE),
        format!("(Kp, Ki) {designed:?} is within 15 % of the lesson's {LESSON_GAINS:?}"),
    );
    if let Some(pair) = designed {
        check_comparison(&mut findings, &result["comparison"], pair);
    }
    findings.check(
        stage_status(&result, "Tune") == Some("complete"),
        "the Tune stage is complete after the result",
    );
    let section = check_stored_tune(&mut findings, &study_of(&lab.scratch.project));
    findings.check(section["method"] == "SIMC", "the Tune section names SIMC");
    findings.conclude();
}
