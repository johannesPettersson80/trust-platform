//! Tests of the model fitted from two selected recordings (`selectedRecordingIdentification.ts`).

#![allow(clippy::cast_precision_loss)] // Sample indices are small counts, far below 2^53.

use super::{fit_selected_recording_model, SelectedRun};
use crate::identification::connected_record::RuntimeVerificationResult;
use crate::identification::independent_validation::RequirementStatus;
use crate::identification::record_fixtures::{build, Row, Spec, Verdict};
use crate::identification::recorded_model::RecordedFitError;
use crate::loop_definition::LoopDefinition;
use crate::recording::opened_channels::RecordingSignalSelection;

const CHANNELS: &[(&str, &str)] = &[("ValveMV", "%"), ("LevelPV", "degC")];

fn loop_definition() -> LoopDefinition {
    crate::recording::fixtures::loop_definition()
}

fn selection(inputs: &[&str], outputs: &[&str]) -> RecordingSignalSelection {
    RecordingSignalSelection {
        inputs: inputs.iter().map(|id| (*id).to_owned()).collect(),
        outputs: outputs.iter().map(|id| (*id).to_owned()).collect(),
    }
}

/// The runs the fit reads, in the order of the records.
fn runs_of(records: &[RuntimeVerificationResult<Verdict>]) -> Vec<SelectedRun<'_>> {
    records.iter().map(SelectedRun::from).collect()
}

fn constant_rows() -> Vec<Row> {
    (0..40_u64)
        .map(|index| (index as f64 * 0.1, index, vec![Some(50.0), Some(20.0)]))
        .collect()
}

fn record_of(id: &str, project_revision: &str, rows: &[Row]) -> RuntimeVerificationResult<Verdict> {
    build(&Spec {
        id,
        project_revision,
        model_revision: "connected-plant:tank",
        kind: "setpoint_step",
        channels: CHANNELS,
        rows,
        baseline: None,
        verdict: RequirementStatus::Pass,
    })
}

fn refusal_of<T>(result: Result<T, RecordedFitError>) -> String {
    match result {
        Err(RecordedFitError::Refused(refusal)) => refusal.0,
        Err(other) => panic!("expected a refusal, got {other:?}"),
        Ok(_) => panic!("expected a refusal, got a fit"),
    }
}

#[test]
fn two_recordings_must_be_independent_and_of_one_project_revision() {
    let rows = constant_rows();
    let same = record_of("same", "sha256:project", &rows);
    let pair = [same.clone(), same];
    let refused = fit_selected_recording_model(
        &loop_definition(),
        &runs_of(&pair),
        &selection(&["ValveMV"], &["LevelPV"]),
    );
    assert_eq!(
        refusal_of(refused),
        "Two independent recordings from the same project revision are required."
    );
    let first = record_of("first", "sha256:project", &rows);
    let other_project = record_of("second", "sha256:other", &rows);
    let pair = [first, other_project];
    let refused = fit_selected_recording_model(
        &loop_definition(),
        &runs_of(&pair),
        &selection(&["ValveMV"], &["LevelPV"]),
    );
    assert_eq!(
        refusal_of(refused),
        "Two independent recordings from the same project revision are required."
    );
}

#[test]
fn a_selected_signal_must_be_in_both_recordings_with_its_unit() {
    let rows = constant_rows();
    let runs = [
        record_of("first", "sha256:project", &rows),
        record_of("second", "sha256:project", &rows),
    ];
    let refused = fit_selected_recording_model(
        &loop_definition(),
        &runs_of(&runs),
        &selection(&["NOPE"], &["LevelPV"]),
    );
    assert_eq!(
        refusal_of(refused),
        "Recording signal NOPE is missing or has inconsistent units."
    );
}

#[test]
fn a_single_pair_without_excitation_falls_to_the_state_fallback() {
    let rows = constant_rows();
    let runs = [
        record_of("first", "sha256:project", &rows),
        record_of("second", "sha256:project", &rows),
    ];
    match fit_selected_recording_model(
        &loop_definition(),
        &runs_of(&runs),
        &selection(&["ValveMV"], &["LevelPV"]),
    ) {
        Err(RecordedFitError::StateModelRequired { attempts }) => {
            assert_eq!(attempts, [] as [String; 0]);
        }
        Err(other) => panic!("expected the state fallback, got {other:?}"),
        Ok(_) => panic!("constant recordings identify no model"),
    }
}

#[test]
fn several_selected_inputs_wait_for_the_state_fit() {
    let rows = constant_rows();
    let runs = [
        record_of("first", "sha256:project", &rows),
        record_of("second", "sha256:project", &rows),
    ];
    match fit_selected_recording_model(
        &loop_definition(),
        &runs_of(&runs),
        &selection(&["ValveMV", "ValveMV"], &["LevelPV"]),
    ) {
        Err(RecordedFitError::Parked { path }) => assert!(path.contains("V7")),
        Err(other) => panic!("expected the parked state fit, got {other:?}"),
        Ok(_) => panic!("two inputs are not fitted here"),
    }
}

/// Two hundred and forty samples of a first-order plant of gain two and a time constant of five seconds around 50 degC, excited by
/// a setpoint step of `size` percent at sample `step_index` and held to the end: the experiment of a `setpoint_step` record
/// (B5.2). The validated fit holds the step from its first change to the next opposite change, and needs twenty samples of it
/// (`minimum_samples`), so a pseudo-random command that changes every five samples would leave it too short to fit.
fn step_rows(step_index: u64, size: f64) -> Vec<Row> {
    let alpha = (-1.0_f64 / 5.0).exp();
    let mut process = 50.0_f64;
    (0..240_u64)
        .map(|index| {
            let command = if index < step_index {
                50.0
            } else {
                50.0 + size
            };
            process = alpha * process + (1.0 - alpha) * (50.0 + 2.0 * (command - 50.0));
            (index as f64, index + 1, vec![Some(command), Some(process)])
        })
        .collect()
}

#[test]
fn a_pair_of_an_isolated_run_and_a_connected_run_is_fitted_with_independent_validation() {
    // An isolated simulation's run keeps the model of its simulation, not the connected plant's. The fit reads both runs the
    // same way (`selectedRecordingIdentification.ts`), and the second run checks the model fitted on the first (B5.2).
    let connected = build(&Spec {
        id: "connected",
        project_revision: "sha256:project",
        model_revision: "connected-plant:tank",
        kind: "setpoint_step",
        channels: CHANNELS,
        rows: &step_rows(10, 5.0),
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    let isolated = build(&Spec {
        id: "isolated",
        project_revision: "sha256:project",
        model_revision: "sha256:simulation",
        kind: "setpoint_step",
        channels: CHANNELS,
        rows: &step_rows(20, -5.0),
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    let pair = [connected, isolated];
    fit_selected_recording_model(
        &loop_definition(),
        &runs_of(&pair),
        &selection(&["ValveMV"], &["LevelPV"]),
    )
    .expect("the isolated run checks the model fitted on the connected run");
}

#[test]
fn an_isolated_run_with_the_identifier_of_the_connected_run_is_not_an_independent_recording() {
    let first = record_of("same", "sha256:project", &step_rows(10, 5.0));
    let isolated = build(&Spec {
        id: "same",
        project_revision: "sha256:project",
        model_revision: "sha256:simulation",
        kind: "setpoint_step",
        channels: CHANNELS,
        rows: &step_rows(20, -5.0),
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    let pair = [first, isolated];
    let refused = fit_selected_recording_model(
        &loop_definition(),
        &runs_of(&pair),
        &selection(&["ValveMV"], &["LevelPV"]),
    );
    assert_eq!(
        refusal_of(refused),
        "Two independent recordings from the same project revision are required."
    );
}
