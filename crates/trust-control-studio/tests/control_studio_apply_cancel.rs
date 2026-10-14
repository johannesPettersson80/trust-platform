//! An Apply interrupted while its response is recorded, and the reconciliation of a parameter operation by its identity, on a real
//! runtime (checklist V4.8; specification 35, §0.1b, B6.3, B6.4; runtime protocol annex E7 and E11; the Apply part of the `TypeScript`
//! journey `control-studio-engineer-workflow.integration.test.ts`).
//!
//! The loop is lesson 30's heater, `examples/control/06-pid-tuning-methods/30-model-based-pi-tuning`, a `Control.PID` whose program
//! starts in Manual, at ten times real time, as `control_studio_apply.rs` runs it. The journey's own loop, the engineer fixture's
//! proportional law, is the program's own law with a declared analysis: both are parked for V7, so the Rust engine refuses to tune it,
//! and its proportional Apply (C13) moves to V7 with it. Apply and Restore themselves are `control_studio_apply.rs`'s journey. Each test
//! here starts its own runtime, so neither depends on what the other left behind:
//! - an Apply interrupted with Ctrl-C while its response is recorded: `identify`, `tune --method simc` and `validate` give the candidate;
//!   the first Ctrl-C cancels the operation, the verb waits for the runtime to confirm the restoration and exits with status 130, the
//!   record and the mode found are back, and the study keeps the cancelled Apply and its restoration (B6.4, §0.1b, A8.1);
//! - the reconciliation of a parameter operation by its identity. The harness cannot drop an answer, so the lost acknowledgement is not
//!   produced. Instead the test applies a record once and reconciles that operation by its identity, which must confirm it without a
//!   second write; then it cancels an operation that was never dispatched and reads the record, which must be unchanged (E7.6, E11.4).
//!
//! A check that does not block a later one is recorded and the test goes on. Run time at ten times real time: under a minute each,
//! nearly all of it the verbs' own work. The file is Unix only: the runtime's endpoint is a Unix socket.
#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::parameter_application::{CancelOutcome, ParameterOwner};
use trust_control_studio::runtime_client::parameters::{
    reconcile_parameter_operation, runtime_binding, ParameterReconciliation, ParameterTransactions,
};

use common::engine::{same_values, study_of, verb_arguments, Findings, Lab, Program};

/// Lesson 30's heater project, a copy of which each test runs (see the module documentation).
const LESSON_30: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning"
);
/// The loop's file in the copy.
const LOOP_FILE: &str = "control-loop.json";
/// The speed of the simulation, ten times real time.
const TIME_SCALE: u32 = 10;
/// A control endpoint that nothing listens on: Tune must finish without reaching it (B4.1).
const UNREACHABLE: &str = "tcp://127.0.0.1:1";
/// The longest that one verb may run; at ten times real time the verbs take seconds, so this only ends a hung run.
const LIMIT: Duration = Duration::from_secs(300);
/// The progress text of an Apply while its response is recorded (`engine_texts::recording_progress`).
const OBSERVATION: &str = "Recording the response";
/// The proportional gain of the controller's parameter record.
const KP: &str = "P1.Parameters.ProportionalGain";
/// The gain that the reconciliation applies once: different from the gain the program designs, so the write is visible.
const RECONCILED_GAIN: f64 = 2.0;

/// The text of a string field, or an empty text.
fn text_of(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_string()
}

/// Records the blocking message of a part, if the part ended with one, as a finding.
fn record(findings: &mut Findings, part: Result<(), String>) {
    if let Err(blocked) = part {
        findings.check(false, blocked);
    }
}

/// Identify, tune and validate: the candidate the later parts apply. Returns the candidate's answer, which holds its gain.
fn candidate_of(lab: &Lab, findings: &mut Findings) -> Result<Value, String> {
    let (program, status) = lab.run("identify", "identify", &["--json"], true, LIMIT);
    if status.code() != Some(0) || program.answer().get("result").is_none() {
        return Err(format!(
            "identify fits the heater and finishes with its result (B3):\n{}",
            program.report(status)
        ));
    }
    let (program, status) = lab.run(
        "tune",
        "tune",
        &["--method", "simc", "--endpoint", UNREACHABLE, "--json"],
        false,
        LIMIT,
    );
    let candidate = program.answer()["result"]["candidate"].clone();
    if status.code() != Some(0) || candidate["applied"] != false {
        return Err(format!(
            "tune designs an unapplied candidate (B4.1):\n{}",
            program.report(status)
        ));
    }
    findings.check(
        candidate["parameters"]["proportionalGain"]
            .as_f64()
            .is_some_and(|gain| gain.is_finite() && gain > 0.0),
        "the candidate's proportional gain is finite and positive (N5)",
    );
    let (program, status) = lab.run("validate", "validate", &["--json"], true, LIMIT);
    if status.code() != Some(0) || program.answer()["result"]["applyAvailable"] != true {
        return Err(format!(
            "validate leaves Apply available (B5.4):\n{}",
            program.report(status)
        ));
    }
    Ok(candidate)
}

/// An Apply interrupted while its response is recorded: the first Ctrl-C cancels it, and the previous settings return (B6.4, §0.1b).
fn interrupted_apply(lab: &Lab, findings: &mut Findings) -> Result<(), String> {
    let project = lab.project.clone();
    let loop_id = lab.loop_definition.id.clone();
    let values_before = lab.parameter_values();
    let mode_before = lab.is_auto();
    let (program, status) = lab.run("apply-review-cancel", "apply", &["--json"], true, LIMIT);
    let identity = text_of(&program.answer()["result"], "identity");
    if status.code() != Some(0) || !identity.starts_with("sha256:") {
        return Err(format!(
            "the review for the interrupted Apply names its identity:\n{}",
            program.report(status)
        ));
    }
    let arguments = verb_arguments(
        "apply",
        &project,
        &loop_id,
        &lab.journal,
        &["--review", identity.as_str(), "--json"],
    );
    let mut interrupted = Program::start(&arguments, true, &lab.folder, "apply-interrupted");
    let observed = interrupted.wait_for_error_text(OBSERVATION, LIMIT);
    findings.check(
        observed,
        "the Apply's response is being recorded before the interrupt (B6.3)",
    );
    if observed {
        interrupted.interrupt();
    }
    let status = interrupted.wait(LIMIT);
    findings.check(
        status.code() == Some(130),
        format!(
            "an interrupted Apply ends with status 130 (§0.1b):\n{}",
            interrupted.report(status)
        ),
    );
    findings.check(
        same_values(&lab.parameter_values(), &values_before),
        "the interrupted Apply restores the gain from before it (B6.4)",
    );
    findings.check(
        lab.is_auto() == mode_before,
        "the interrupted Apply returns the loop to the mode it was found in (A6.2)",
    );
    let study = study_of(&project, &loop_id);
    let applications = study["sections"]["apply"]["applications"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let restorations = study["sections"]["apply"]["restorations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    findings.check(
        applications
            .last()
            .is_some_and(|receipt| receipt["result"]["status"] == "cancelled"),
        format!("the study keeps the cancelled Apply's receipt (A8.1): {applications:?}"),
    );
    findings.check(
        restorations
            .last()
            .is_some_and(|receipt| receipt["restored"] == true),
        format!("the study keeps the confirmed restoration of the cancelled Apply (A8.1): {restorations:?}"),
    );
    Ok(())
}

/// The reconciliation of a parameter operation by its identity on the runtime: the record confirms the operation without a second write,
/// and a cancel of an operation that was never dispatched leaves the record as it was (E7.6, E11.4).
fn reconcile(lab: &Lab, findings: &mut Findings) -> Result<(), String> {
    let binding = lab
        .loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(|| "the loop declares no parameter binding".to_string())?;
    let mut client = lab.client();
    let mut transactions = ParameterTransactions::new(
        &mut client,
        runtime_binding(binding),
        ApplySource::Simulation,
    );
    // A review takes exactly the declared parameters: the record as the program designed it, with the gain changed.
    let mut values: BTreeMap<String, f64> = lab.parameter_values();
    values.insert(KP.to_string(), RECONCILED_GAIN);
    let review = transactions.review(&values).map_err(|failure| {
        format!(
            "the record is reviewed against the runtime: {}",
            failure.message
        )
    })?;
    let applied = transactions
        .apply(&review, false)
        .map_err(|failure| format!("the record is applied in Manual: {}", failure.message))?;
    findings.check(
        applied.transaction_id.as_deref() == Some(review.transaction_id.as_str())
            && same_values(&applied.values, &values),
        "the runtime confirms the applied record under the review's identity (E7.3)",
    );
    let reconciled = reconcile_parameter_operation(&mut transactions, &review, None);
    findings.check(
        matches!(&reconciled, Ok(ParameterReconciliation::Confirmed { observed, .. }) if observed.transaction_id == applied.transaction_id),
        format!("the operation is reconciled by its identity, as confirmed (E11.4): {reconciled:?}"),
    );
    let after = transactions
        .status()
        .map_err(|failure| format!("the record reads: {}", failure.message))?;
    findings.check(
        after.transaction_id == applied.transaction_id && same_values(&after.values, &values),
        "reconciling the operation writes nothing: the record still holds the one transaction (E11.4)",
    );
    let owner = ParameterOwner {
        id: "parameter-never-dispatched".to_string(),
        program: review.observed.program_revision.clone(),
        binding: review.observed.binding_revision.clone(),
    };
    let cancelled = transactions.cancel_owner(&owner);
    findings.check(
        matches!(&cancelled, Ok(CancelOutcome::BeforeDispatch)),
        format!("a cancel of an operation that was never dispatched is answered before dispatch (E7.6): {cancelled:?}"),
    );
    let read_back = transactions
        .status()
        .map_err(|failure| format!("the record reads: {}", failure.message))?;
    findings.check(
        read_back.transaction_id == applied.transaction_id
            && same_values(&read_back.values, &values),
        "the cancel and the status read leave the record as it was (E7.6)",
    );
    Ok(())
}

/// An Apply interrupted while its response is recorded restores the previous settings (V4.8, B6.4).
///
/// Run time: ×10, under a minute.
#[test]
fn an_apply_interrupted_while_its_response_is_recorded_restores_the_previous_settings() {
    let lab = Lab::start("apply-cancel", LESSON_30, LOOP_FILE, TIME_SCALE);
    let mut findings = Findings::default();
    let interrupted =
        candidate_of(&lab, &mut findings).and_then(|_| interrupted_apply(&lab, &mut findings));
    record(&mut findings, interrupted);
    findings.conclude();
}

/// A parameter operation is reconciled by its identity without a second write, and a cancel before dispatch leaves the record as it was
/// (V4.8, E7.6, E11.4).
///
/// Run time: ×10, a few seconds after the runtime starts.
#[test]
fn a_parameter_operation_is_reconciled_by_its_identity_without_a_second_write() {
    let lab = Lab::start("reconcile", LESSON_30, LOOP_FILE, TIME_SCALE);
    let mut findings = Findings::default();
    let reconciled = reconcile(&lab, &mut findings);
    record(&mut findings, reconciled);
    findings.conclude();
}
