//! The identification methods of `identify --method` against a real runtime, through the built program `trust-control-studio`
//! (checklist V1.7; specification 35, §0.1b, B3.3 to B3.5 and B3.9; runtime protocol annex E5.2 to E5.7 and E11.2).
//!
//! Each test copies an example whose loop permits its method, runs `trust-runtime run --simulation` on the copy at ten times real
//! time, and runs `identify --method <word> --json` against it. The harness is `identify_lab/mod.rs`.
//!
//! # Which example permits which method
//!
//! `identify --method` runs the loop's declared settings with the method asked for (`declared_settings`). The engine refuses a
//! method that the loop does not permit (`method_applicability`, `validate_explicit_method`) and a method whose mode the loop is
//! not in (`identification/mode.rs`: every method but the setpoint step needs Manual, the setpoint step needs Auto, and the mode
//! transfer that would change the mode comes with stage V4).
//! - Lesson 30 (`examples/control/06-pid-tuning-methods/30-model-based-pi-tuning`) is the heater of `identify_lab`, in Manual at
//!   75 %. Its loop permits the open-loop methods: step, pulse, PRBS and multisine. It declares no relay direction, so it does
//!   not permit the relay.
//! - Lesson 29 (`examples/control/06-pid-tuning-methods/29-relay-tuning`) is the same heater, also in Manual at 75 %. Its loop
//!   declares `relayDirection: positive`, so it permits the relay. The relay needs Manual, as every method but the setpoint step
//!   does, so a loop in Auto would be refused for its mode.
//! - No control example in the repository starts in Auto with both a setpoint and a parameter binding, so no setpoint step can run
//!   in this stage. Its test asserts the refusal that the engine gives in Manual (B3.9: a mode change refused, "as the reason states").
//!
//! # What every method that runs must give
//!
//! Exit 0 with a provisional model of a plausible family (first order, with or without its delay) and a positive gain. The study
//! keeps the Identify section and the one complete recording that it names. The experiment that the journal held ends completed
//! and restored, the journal is empty, and the controller command is back at its value before the experiment. The numbers of the
//! fit are not asserted per method: `control_studio_identify.rs` asserts them for the step.
//!
//! # Running
//!
//! `cargo test -p trust-control-studio --test control_studio_identify_methods`, after `cargo build -p trust-runtime --bin
//! trust-runtime`, or with `TRUST_RUNTIME_BIN` naming the executable. The file is Unix only, as the other runtime tests are: the
//! runtime's endpoint is a Unix socket.
#![cfg(unix)]

mod common;
mod identify_lab;

use serde_json::Value;
use trust_control::experiment::{ControlExperimentKind, ControlExperimentPlan, ExperimentValue};

use identify_lab::{
    check_no_model_kept, check_plausible_model, check_plot, check_study_keeps_the_model, result_of,
    result_recording_id, stage_status, Findings, Lab, Program, HOLDING_COMMAND, LESSON_30,
    LESSON_30_LOOP, RUN_LIMIT,
};

/// The relay of lesson 29, `examples/control/06-pid-tuning-methods/29-relay-tuning`: the same heater, in Manual, with its
/// process direction declared (module documentation).
const LESSON_29: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/29-relay-tuning"
);
/// The identifier of lesson 29's loop (`control-loop.json`).
const LESSON_29_LOOP: &str = "control-relay-tuning";

/// The `failure` of the answer that a verb printed with `--json` (§0.1b: `{"failure": …}`), or `Value::Null` when it printed none.
fn failure_of(program: &Program) -> Value {
    serde_json::from_str::<Value>(&program.stdout())
        .ok()
        .and_then(|answer| answer.get("failure").cloned())
        .unwrap_or_default()
}

/// Runs `identify --method <word> --json` on `lab` and follows it to its end, and checks what every method that runs must give
/// (module documentation). Returns the plan of the experiment that the journal held while it ran, for the method's own checks;
/// `None` when the journal never held one.
fn identify_by_method(
    findings: &mut Findings,
    lab: &Lab,
    word: &str,
) -> Option<ControlExperimentPlan> {
    let original = lab.controller_command();
    let mut identify = lab.start_verb("identify", &["--method", word, "--json"], word);
    let mut plan = None;
    let status = identify.wait_polling(RUN_LIMIT, || {
        if plan.is_none() {
            plan = lab.journaled_plan();
        }
    });
    findings.check(
        status.code() == Some(0),
        format!(
            "identify --method {word} should finish with its result (status 0):\n{}",
            lab.describe(&identify, status)
        ),
    );
    let result = result_of(findings, lab, &identify, status);
    let recording_id = result_recording_id(findings, &result);
    findings.check(
        stage_status(&result, "Identify") == Some("complete"),
        format!(
            "the Identify stage should be complete after the result, it is {:?}",
            stage_status(&result, "Identify")
        ),
    );
    check_plot(findings, &result);
    let section =
        check_study_keeps_the_model(findings, lab, recording_id.as_deref().unwrap_or("(none)"));
    check_plausible_model(findings, &result, &section);
    lab.check_journal_empty(findings, "once the identification has settled");
    if let Some(experiment) = &plan {
        let record = lab.check_ended_and_restored(findings, &experiment.id);
        findings.check(
            record
                .as_ref()
                .is_some_and(|record| record["status"] == "completed"),
            format!("the experiment should end normally (status completed): {record:?}"),
        );
    } else {
        findings.check(
            false,
            "the journal should hold the campaign's step while its experiment runs (E11.2)",
        );
    }
    lab.check_command_is(findings, original);
    plan
}

/// `identify --method step` on lesson 30 runs one plant-input step in Manual, holds it until the response settles, and restores
/// the input (B3.3, B3.4 steps 3 to 5; annex E5.2: a step has one point, held until its restoration). It replaces the `mv_step`
/// case of the `TypeScript` journey "all identification methods", and the first test of "single identification experiment" (one
/// Identify, a provisional model, and the measured experiment kept). At ten times real time the experiment holds for about 2 s of
/// wall time, and the test takes about 8 s with the build and the start of the runtime.
#[test]
fn step_identifies_the_heater_from_one_plant_input_step() {
    let lab = Lab::start_example("step", LESSON_30, LESSON_30_LOOP);
    let mut findings = Findings::default();
    let before = lab.controller_command();
    findings.check(
        (before - HOLDING_COMMAND).abs() < 1e-6,
        format!(
            "the heater should hold {HOLDING_COMMAND} % in Manual before the step, it reads {before} %"
        ),
    );
    let plan = identify_by_method(&mut findings, &lab, "step");
    findings.check(
        plan.as_ref().is_some_and(|experiment| {
            experiment.kind == ControlExperimentKind::MvStep && experiment.trajectory.len() == 1
        }),
        format!(
            "a step should hold one point until its restoration (E5.2), the journal holds {:?}",
            plan.as_ref()
                .map(|experiment| (experiment.kind, experiment.trajectory.len()))
        ),
    );
    findings.conclude();
}

/// `identify --method pulse` on lesson 30 raises the manual output and returns it inside the record: the pulse's second point is
/// its restoration, and it comes before the end of the experiment (annex E5.2). It replaces the `mv_pulse` case of "all
/// identification methods", which asserts that the pulse restores during the record. At ten times real time the test takes about
/// 10 s: the pulse runs most of its 60 s declared duration, 6 s of wall time.
#[test]
fn pulse_identifies_the_heater_and_returns_its_input_inside_the_record() {
    let lab = Lab::start_example("pulse", LESSON_30, LESSON_30_LOOP);
    let mut findings = Findings::default();
    if let Some(plan) = identify_by_method(&mut findings, &lab, "pulse") {
        findings.check(
            plan.kind == ControlExperimentKind::MvPulse,
            format!(
                "the experiment should be a pulse of the manual output, it is {:?}",
                plan.kind
            ),
        );
        let returned_inside = match plan.restoration_value {
            ExperimentValue::Number(restoration) => plan.trajectory.iter().any(|point| {
                point.at_ms > 0.0
                    && point.at_ms < plan.duration_ms
                    && (point.value - restoration).abs() < 1e-9
            }),
            ExperimentValue::Boolean(_) => false,
        };
        findings.check(
            returned_inside,
            "the pulse should return its input to the restoration value before the end of the record (annex E5.2)",
        );
    }
    findings.conclude();
}

/// `identify --method prbs` on lesson 30 excites the manual output with a pseudo-random binary sequence of more than fifteen
/// points, over the declared duration (annex E5.2). It replaces the `prbs` case of "all identification methods", which asserts the
/// same trajectory. At ten times real time the experiment runs its full 60 s declared duration, 6 s of wall time, and the test
/// takes about 10 s.
#[test]
fn prbs_identifies_the_heater_from_a_pseudo_random_sequence() {
    let lab = Lab::start_example("prbs", LESSON_30, LESSON_30_LOOP);
    let mut findings = Findings::default();
    if let Some(plan) = identify_by_method(&mut findings, &lab, "prbs") {
        let points = plan.trajectory.len();
        findings.check(
            plan.kind == ControlExperimentKind::Prbs && points > 15,
            format!(
                "the experiment should be a PRBS with more than 15 points, it is {:?} with {points} points",
                plan.kind
            ),
        );
    }
    findings.conclude();
}

/// `identify --method multisine` on lesson 30 excites the manual output with a multisine of more than fifteen points, over the
/// declared duration (annex E5.2). It replaces the `multisine` case of "all identification methods", which asserts the same
/// trajectory. At ten times real time the test takes about 10 s, as the PRBS test does.
#[test]
fn multisine_identifies_the_heater_from_a_multisine_excitation() {
    let lab = Lab::start_example("multisine", LESSON_30, LESSON_30_LOOP);
    let mut findings = Findings::default();
    if let Some(plan) = identify_by_method(&mut findings, &lab, "multisine") {
        let points = plan.trajectory.len();
        findings.check(
            plan.kind == ControlExperimentKind::Multisine && points > 15,
            format!(
                "the experiment should be a multisine with more than 15 points, it is {:?} with {points} points",
                plan.kind
            ),
        );
    }
    findings.conclude();
}

/// `identify --method relay` on lesson 29 switches the manual output from the measured process value, not by a timed square wave
/// (annex E5.2: a relay has no trajectory and carries its relay feedback). It replaces the `relay` case of "all identification
/// methods", which asserts the same. At ten times real time the relay runs its 60 s declared duration, 6 s of wall time, and the
/// test takes about 10 s.
#[test]
fn relay_identifies_the_heater_from_a_relay_on_the_measurement() {
    let lab = Lab::start_example("relay", LESSON_29, LESSON_29_LOOP);
    let mut findings = Findings::default();
    if let Some(plan) = identify_by_method(&mut findings, &lab, "relay") {
        let has_relay_feedback = plan.relay_feedback.is_some();
        findings.check(
            plan.kind == ControlExperimentKind::Relay
                && has_relay_feedback
                && plan.trajectory.is_empty(),
            format!(
                "the experiment should be a relay on the measured value, with no timed trajectory: it is {:?}, with a relay feedback: {has_relay_feedback}",
                plan.kind
            ),
        );
    }
    findings.conclude();
}

/// `identify --method setpoint-step` on lesson 30, whose loop is in Manual, is refused before anything is recorded or written:
/// status 1, the engine's words for the mode that the method needs (B3.9: "Mode change refused ... As the reason states"), no
/// journal entry, no model, and the controller command unchanged. The `TypeScript` journey "all identification methods" runs
/// `setpoint_step` on the default control project, which also starts in Manual; its engine changes the mode by a transfer
/// (`runtimeTransfer.ts`) that this engine receives with stage V4, so this stage asserts the refusal the engine gives. At ten times
/// real time the test takes about 4 s: the copy, the build, the start of the runtime and a refusal at the first scan.
#[test]
fn setpoint_step_is_refused_in_manual_before_anything_is_recorded() {
    let lab = Lab::start_example("refusal", LESSON_30, LESSON_30_LOOP);
    let mut findings = Findings::default();
    let original = lab.controller_command();
    findings.check(
        (original - HOLDING_COMMAND).abs() < 1e-6,
        format!(
            "the heater should hold {HOLDING_COMMAND} % in Manual before the refusal, it reads {original} %"
        ),
    );
    let mut identify = lab.start_verb(
        "identify",
        &["--method", "setpoint-step", "--json"],
        "setpoint",
    );
    let status = identify.wait(RUN_LIMIT);
    findings.check(
        status.code() == Some(1),
        format!(
            "a method that the loop's mode does not permit should be refused with status 1:\n{}",
            lab.describe(&identify, status)
        ),
    );
    let failure = failure_of(&identify);
    findings.check(
        failure["message"] == "Identify does not switch the loop's mode yet.",
        format!("the refusal should name the mode change: {failure}"),
    );
    findings.check(
        failure["detail"]
            == "Identify does not switch the loop's mode yet. Put the loop in Auto, or choose Automatic.",
        format!("the refusal should say which mode the method needs, as B3.9 requires: {failure}"),
    );
    lab.check_journal_empty(&mut findings, "after the refusal");
    check_no_model_kept(&mut findings, &lab);
    lab.check_command_is(&mut findings, original);
    findings.conclude();
}
