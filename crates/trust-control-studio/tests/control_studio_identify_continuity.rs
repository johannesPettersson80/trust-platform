//! An identification's continuity across the end of its engine process, against a real runtime, through the built program
//! `trust-control-studio` (checklist V1.7; specification 35, §0.1b, A8.5, A8.6 and B3.4; runtime protocol annex E5.5 to E5.7 and
//! E11.2 to E11.6).
//!
//! Each test identifies the heater of lesson 30 (`examples/control/06-pid-tuning-methods/30-model-based-pi-tuning`) at ten times
//! real time, waits until the runtime runs the experiment, and then ends the program outright with SIGKILL, as a crash would. The
//! program cancels nothing when it is killed: the journal and the runtime keep the operation (A8.5). The harness is
//! `identify_lab/mod.rs`; the tests of the methods are in `control_studio_identify_methods.rs`.
//!
//! # Running
//!
//! `cargo test -p trust-control-studio --test control_studio_identify_continuity`, after `cargo build -p trust-runtime --bin
//! trust-runtime`, or with `TRUST_RUNTIME_BIN` naming the executable. The file is Unix only, as the other runtime tests are: the
//! runtime's endpoint is a Unix socket, and the kill is a signal.
#![cfg(unix)]

mod common;
mod identify_lab;

use std::process::ExitStatus;
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_control::experiment::ControlExperimentPlan;
use trust_control::identification::reconcile::StatusRead;
use trust_control_studio::journal::{Journal, JournalEntry};
use trust_control_studio::runtime_client::RuntimeClient;

use identify_lab::{
    check_no_model_kept, check_plausible_model, check_plot, check_study_keeps_the_model, parameter,
    result_of, result_recording_id, stage_status, Findings, Lab, Program, HOLDING_COMMAND,
    LESSON_30, LESSON_30_LOOP, RUN_LIMIT,
};

/// The longest the tests wait for the experiment to run in the runtime, after the program has started it.
const EXPERIMENT_LIMIT: Duration = Duration::from_secs(120);
/// How often a test looks at the program, the journal and the runtime while it waits.
const POLL: Duration = Duration::from_millis(25);
/// The plant's dominant time constant in s, `-1 / lambda` for `A = [[-0.2]]`, and its gain in degC per % (the module
/// documentation of `control_studio_identify.rs`).
const PLANT_TIME_CONSTANT: f64 = 5.0;
const PLANT_GAIN: f64 = 0.2;

/// Whether the runtime reports the experiment `id` as running (E5.6).
fn experiment_running(lab: &Lab, id: &str) -> bool {
    matches!(
        RuntimeClient::read_experiment_status(&lab.connection, id),
        StatusRead::Answer(answer) if answer["status"] == "running"
    )
}

/// Waits until the runtime's record of experiment `id` names the value it applied (E5.6).
fn wait_until_applied(lab: &Lab, id: &str) {
    let begun = Instant::now();
    loop {
        if let StatusRead::Answer(answer) =
            RuntimeClient::read_experiment_status(&lab.connection, id)
            && !answer["last_applied"].is_null()
        {
            return;
        }
        assert!(
            begun.elapsed() < EXPERIMENT_LIMIT,
            "experiment {id} applied nothing within {} s",
            EXPERIMENT_LIMIT.as_secs()
        );
        sleep(POLL);
    }
}

/// Waits until the experiment of the running `identify` is running in the runtime: the journal holds its plan before the start is
/// dispatched (E11.2), and the runtime's record says `running` (E5.6). Returns the plan. The program must still be alive.
fn wait_until_running(lab: &Lab, identify: &mut Program) -> ControlExperimentPlan {
    let begun = Instant::now();
    loop {
        if let Some(status) = identify.child.try_wait().expect("poll the program") {
            panic!(
                "the program ended before its experiment ran:\n{}",
                lab.describe(identify, status)
            );
        }
        let running = lab
            .journaled_plan()
            .filter(|plan| experiment_running(lab, &plan.id));
        if let Some(plan) = running {
            return plan;
        }
        assert!(
            begun.elapsed() < EXPERIMENT_LIMIT,
            "no experiment ran within {} s; the program reports:\n{}",
            EXPERIMENT_LIMIT.as_secs(),
            identify.stderr()
        );
        sleep(POLL);
    }
}

/// The operation that the loop's journal holds for the identification campaign, which is the campaign's recording identifier
/// (E11.2). While a step runs the journal names two operations, the campaign and its experiment, so `cancel` needs this one
/// named (D6). `None` when the journal holds no campaign.
fn campaign_operation(lab: &Lab) -> Option<String> {
    let journal = Journal::open(
        &lab.journal,
        &lab.loop_definition.project_id,
        &lab.loop_definition.id,
    )
    .ok()?;
    journal.entries().iter().find_map(|entry| match entry {
        JournalEntry::IdentificationCampaign { .. } => Some(entry.operation_id().to_string()),
        JournalEntry::RecordingOwner { .. }
        | JournalEntry::Experiment { .. }
        | JournalEntry::IsolatedJob { .. }
        | JournalEntry::Commissioning { .. }
        | JournalEntry::IdentificationMode { .. } => None,
    })
}

/// Ends `program` with SIGKILL, as a crash of the engine would: nothing is cancelled, and the runtime keeps its experiment
/// (A8.5). Returns how the process ended, which has no exit code when a signal ended it.
fn kill_outright(program: &mut Program) -> ExitStatus {
    program.child.kill().expect("send SIGKILL to the program");
    program.child.wait().expect("reap the killed program")
}

/// Checks the heater's gain and time constant of a fit: a gain within 25 % of the plant's, a time constant within a factor of
/// three of the plant's, and one time constant only, as a first-order model has (the tolerances of `control_studio_identify.rs`).
fn check_heater_numbers(findings: &mut Findings, section: &Value) {
    let gain = parameter(section, "Gain");
    findings.check(
        gain.is_some_and(|gain| (PLANT_GAIN * 0.75..=PLANT_GAIN * 1.25).contains(&gain)),
        format!(
            "the gain should be within 25 % of the plant's {PLANT_GAIN} degC per %, it is {gain:?}"
        ),
    );
    let time_constant = parameter(section, "Time constant 1");
    findings.check(
        time_constant.is_some_and(|tau| {
            (PLANT_TIME_CONSTANT / 3.0..=PLANT_TIME_CONSTANT * 3.0).contains(&tau)
        }),
        format!(
            "the time constant should be within a factor of three of the plant's {PLANT_TIME_CONSTANT} s, it is {time_constant:?}"
        ),
    );
    findings.check(
        parameter(section, "Time constant 2").is_none(),
        "a first-order model has one time constant",
    );
}

/// `identify --resume` follows a killed identification to its end (A8.5, A8.6). Lesson 30 is identified, and the program is
/// killed outright once the runtime runs its experiment. The journal then holds the campaign and the runtime still runs the
/// experiment. `identify --resume` follows the experiment to its end, fits its recording, keeps the recording, empties the
/// journal, and the controller output is back at its original command. It replaces, in part, the `TypeScript` journey
/// "identification continuity": the reattachment after the end of the engine, but not its method sequence or its reopening of
/// the panel. At ten times real time the experiment holds for about 2 s of wall time, and the test takes about 10 s with the build
/// and the start of the runtime.
#[test]
fn killed_identification_resumes_from_its_journal_and_follows_its_experiment() {
    let lab = Lab::start_example("resume", LESSON_30, LESSON_30_LOOP);
    let mut findings = Findings::default();
    let original = lab.controller_command();
    findings.check(
        (original - HOLDING_COMMAND).abs() < 1e-6,
        format!(
            "the heater should hold {HOLDING_COMMAND} % in Manual before the step, it reads {original} %"
        ),
    );

    // 1. The identification runs its experiment, and the program is killed while the runtime runs it.
    let mut identify = lab.start_verb("identify", &["--json"], "identify");
    let plan = wait_until_running(&lab, &mut identify);
    let killed = kill_outright(&mut identify);
    findings.check(
        killed.code().is_none(),
        format!("the program should have been ended by a signal, it ended with {killed}"),
    );
    findings.check(
        lab.journaled_plan()
            .is_some_and(|journaled| journaled.id == plan.id),
        "the journal should still hold the campaign and its experiment after the program was killed (A8.5)",
    );
    findings.check(
        experiment_running(&lab, &plan.id),
        "the runtime should still run the experiment after the program was killed (A8.5, A8.6)",
    );

    // 2. `identify --resume` follows the experiment to its end, fits it and keeps the recording.
    let mut resume = lab.start_verb("identify", &["--resume", "--json"], "resume");
    let status = resume.wait(RUN_LIMIT);
    findings.check(
        status.code() == Some(0),
        format!(
            "identify --resume should finish with its result (status 0):\n{}",
            lab.describe(&resume, status)
        ),
    );
    let result = result_of(&mut findings, &lab, &resume, status);
    let recording_id = result_recording_id(&mut findings, &result);
    findings.check(
        stage_status(&result, "Identify") == Some("complete"),
        format!(
            "the Identify stage should be complete after the resumed result, it is {:?}",
            stage_status(&result, "Identify")
        ),
    );
    check_plot(&mut findings, &result);
    // A resumed campaign answers what a fresh one does (wrong behaviour caught: it answered neither the kept recording's samples nor
    // the restoration, so the page drew no recorded signals after a resume, C12).
    findings.check(
        result["recordedSamples"]
            .as_array()
            .is_some_and(|samples| !samples.is_empty()),
        "the resumed result carries the kept recording's samples, as a fresh run's does",
    );
    findings.check(
        result["restoration"] == "restored",
        format!(
            "the resumed result names the restoration its step confirmed: {}",
            result["restoration"]
        ),
    );
    let section = check_study_keeps_the_model(
        &mut findings,
        &lab,
        recording_id.as_deref().unwrap_or("(none)"),
    );
    check_plausible_model(&mut findings, &result, &section);
    check_heater_numbers(&mut findings, &section);
    let record = lab.check_ended_and_restored(&mut findings, &plan.id);
    findings.check(
        record
            .as_ref()
            .is_some_and(|record| record["status"] == "completed"),
        format!("the experiment should end normally (status completed): {record:?}"),
    );
    lab.check_journal_empty(&mut findings, "once the resumed identification has settled");
    lab.check_command_is(&mut findings, original);
    findings.conclude();
}

/// `cancel` ends a killed identification through its journal (§0.1b, E5.5, E11.6). Lesson 30 is identified, and the program is
/// killed outright once the runtime runs its experiment. `cancel` then aborts the experiment through its restoration path and
/// confirms the restoration, keeps the recording, empties the journal, and keeps no model: the study has no Identify section.
/// The test names the campaign's operation, since the journal holds two operations while the step runs (D6). It replaces, in
/// part, the `TypeScript` journey "identification continuity", for the case of an identification that does not come back. At ten
/// times real time the test takes about 8 s with the build and the start of the runtime, the identification up to its experiment,
/// and the abort.
#[test]
fn killed_identification_is_cancelled_through_its_journal_and_keeps_no_model() {
    let lab = Lab::start_example("cancel", LESSON_30, LESSON_30_LOOP);
    let mut findings = Findings::default();
    let original = lab.controller_command();

    // 1. The identification runs its experiment, and the program is killed while the runtime runs it,
    // once the experiment has applied its step: a step that was never applied has nothing to restore,
    // and the record then says `not_required` (E5.5).
    let mut identify = lab.start_verb("identify", &["--json"], "identify");
    let plan = wait_until_running(&lab, &mut identify);
    wait_until_applied(&lab, &plan.id);
    let killed = kill_outright(&mut identify);
    findings.check(
        killed.code().is_none(),
        format!("the program should have been ended by a signal, it ended with {killed}"),
    );
    let Some(operation) = campaign_operation(&lab) else {
        panic!("the journal should hold the campaign after the program was killed (A8.5)");
    };

    // 2. `cancel` names the campaign's operation and follows the cancellation to its end.
    let mut cancel = lab.start_verb("cancel", &[operation.as_str(), "--json"], "cancel");
    let status = cancel.wait(RUN_LIMIT);
    findings.check(
        status.code() == Some(0),
        format!(
            "cancel should finish with its result (status 0):\n{}",
            lab.describe(&cancel, status)
        ),
    );
    let result = result_of(&mut findings, &lab, &cancel, status);
    findings.check(
        result["cancelled"] == json!([operation]),
        format!(
            "cancel should name the one operation it cancelled, it names {}",
            result["cancelled"]
        ),
    );
    let record = lab.check_ended_and_restored(&mut findings, &plan.id);
    findings.check(
        record.as_ref().is_some_and(|record| {
            record["status"] == "aborted"
                && record["stop_reason"] == "user_abort"
                && record["restoration"] == "restored"
        }),
        format!(
            "the running experiment should end aborted by the user, with its restoration confirmed (E5.5, E5.7): {record:?}"
        ),
    );
    lab.check_journal_empty(&mut findings, "once the cancellation has settled");
    check_no_model_kept(&mut findings, &lab);
    lab.check_command_is(&mut findings, original);
    findings.conclude();
}
