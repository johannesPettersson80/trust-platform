//! Tests of the model fitted from two selected recordings (`selectedRecordingIdentification.ts`).

#![allow(clippy::cast_precision_loss)] // Sample indices are small counts, far below 2^53.

use super::fit_selected_recording_model;
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
    let refused = fit_selected_recording_model(
        &loop_definition(),
        &[same.clone(), same],
        &selection(&["ValveMV"], &["LevelPV"]),
    );
    assert_eq!(
        refusal_of(refused),
        "Two independent recordings from the same project revision are required."
    );
    let first = record_of("first", "sha256:project", &rows);
    let other_project = record_of("second", "sha256:other", &rows);
    let refused = fit_selected_recording_model(
        &loop_definition(),
        &[first, other_project],
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
        &runs,
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
        &runs,
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
        &runs,
        &selection(&["ValveMV", "ValveMV"], &["LevelPV"]),
    ) {
        Err(RecordedFitError::Parked { path }) => assert!(path.contains("V7")),
        Err(other) => panic!("expected the parked state fit, got {other:?}"),
        Ok(_) => panic!("two inputs are not fitted here"),
    }
}
