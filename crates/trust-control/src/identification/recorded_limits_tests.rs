//! Tests of the limits check of a connected recording (specification 35, B3.4 step 6 and B3.9): the plant input and the
//! measured output are checked over the whole recording, and the first problem names what was found.

use std::collections::BTreeMap;

use super::evaluate_recorded_limits;
use crate::identification::connected_record::{
    BoundsCheck, BoundsKind, BoundsWindow, RunConditions, RunEvidence, ValidationChannel,
    ValidationRun, ValidationSample, ValidationValue,
};
use crate::identification::independent_validation::{RequirementStatus, RunEvaluation};
use crate::recording::channels::ChannelQuality;

const PLANT_INPUT: &str = "ValveMV";
const MEASURED_OUTPUT: &str = "LevelPV";

fn good(value: f64) -> ValidationValue {
    ValidationValue {
        value: Some(value),
        quality: ChannelQuality::Good,
    }
}

fn reading(value: Option<f64>, quality: ChannelQuality) -> ValidationValue {
    ValidationValue { value, quality }
}

/// A run of samples, each a time with the plant input's and the measured output's values, on the loop's two channels.
fn run(rows: &[(f64, ValidationValue, ValidationValue)]) -> ValidationRun {
    let samples: Vec<ValidationSample> = rows
        .iter()
        .enumerate()
        .map(|(index, (time, input, output))| ValidationSample {
            time_seconds: *time,
            sequence: u64::try_from(index).expect("a few samples"),
            values: BTreeMap::from([
                (PLANT_INPUT.to_owned(), *input),
                (MEASURED_OUTPUT.to_owned(), *output),
            ]),
        })
        .collect();
    ValidationRun {
        id: "recording-1".to_owned(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds: samples.last().map_or(0.0, |sample| sample.time_seconds),
            sample_times_seconds: samples.iter().map(|sample| sample.time_seconds).collect(),
            seed: 1,
            model_revision: "connected-plant:sha256:p".to_owned(),
            program_revision: "sha256:p".to_owned(),
            parameter_schema_revision: "sha256:l".to_owned(),
        },
        channels: vec![
            ValidationChannel {
                id: PLANT_INPUT.to_owned(),
                unit: "%".to_owned(),
            },
            ValidationChannel {
                id: MEASURED_OUTPUT.to_owned(),
                unit: "degC".to_owned(),
            },
        ],
        samples,
    }
}

fn bounds(channel: &str, minimum: f64, maximum: f64, end: f64) -> BoundsCheck {
    BoundsCheck {
        id: format!("bounds:{channel}"),
        kind: BoundsKind::Bounds,
        channel: channel.to_owned(),
        window: BoundsWindow { start: 0.0, end },
        minimum,
        maximum,
    }
}

/// The checks of the connected record: the plant input within 0 to 100 %, the measured output within 0 to 100 degC.
fn checks(end: f64) -> Vec<BoundsCheck> {
    vec![
        bounds(PLANT_INPUT, 0.0, 100.0, end),
        bounds(MEASURED_OUTPUT, 0.0, 100.0, end),
    ]
}

#[test]
fn a_recording_within_its_limits_passes_with_its_limits_inclusive() {
    let recorded = run(&[
        (0.0, good(30.0), good(45.0)),
        (1.0, good(60.0), good(50.0)),
        (2.0, good(100.0), good(0.0)),
    ]);
    let evaluation = evaluate_recorded_limits(&recorded, &checks(2.0)).expect("the checks hold");
    assert_eq!(evaluation.status, RequirementStatus::Pass);
    assert_eq!(evaluation.finding, None);
    assert_eq!(evaluation.specification, checks(2.0));
    assert_eq!(evaluation.status(), RequirementStatus::Pass);
}

#[test]
fn a_measured_output_above_its_range_fails_naming_the_signal_time_value_and_bound() {
    let recorded = run(&[
        (0.0, good(30.0), good(45.0)),
        (2.5, good(30.0), good(104.0)),
        (3.0, good(30.0), good(120.0)),
    ]);
    let evaluation = evaluate_recorded_limits(&recorded, &checks(3.0)).expect("the checks hold");
    assert_eq!(evaluation.status, RequirementStatus::Fail);
    assert_eq!(
        evaluation.finding.as_deref(),
        Some("LevelPV is 104 degC at 2.5 s, above its maximum of 100 degC.")
    );
    assert_eq!(evaluation.finding(), evaluation.finding.as_deref());
}

#[test]
fn a_plant_input_below_its_range_fails_naming_the_signal_time_value_and_bound() {
    let recorded = run(&[(0.0, good(30.0), good(45.0)), (1.0, good(-3.0), good(45.0))]);
    let evaluation = evaluate_recorded_limits(&recorded, &checks(1.0)).expect("the checks hold");
    assert_eq!(evaluation.status, RequirementStatus::Fail);
    assert_eq!(
        evaluation.finding.as_deref(),
        Some("ValveMV is -3 % at 1 s, below its minimum of 0 %.")
    );
}

#[test]
fn a_bad_sample_is_not_a_pass_and_names_the_signal_and_the_time() {
    let recorded = run(&[
        (0.0, good(30.0), good(45.0)),
        (1.0, good(30.0), reading(Some(50.0), ChannelQuality::Bad)),
    ]);
    let evaluation = evaluate_recorded_limits(&recorded, &checks(1.0)).expect("the checks hold");
    assert_eq!(evaluation.status, RequirementStatus::Inconclusive);
    assert_eq!(
        evaluation.finding.as_deref(),
        Some("LevelPV has no good value at 1 s: bad.")
    );
}

#[test]
fn a_missing_or_forced_sample_is_not_a_pass_either() {
    let missing = run(&[
        (0.0, good(30.0), good(45.0)),
        (2.0, reading(None, ChannelQuality::Good), good(45.0)),
    ]);
    let evaluation = evaluate_recorded_limits(&missing, &checks(2.0)).expect("the checks hold");
    assert_eq!(evaluation.status, RequirementStatus::Inconclusive);
    assert_eq!(
        evaluation.finding.as_deref(),
        Some("ValveMV has no good value at 2 s: missing.")
    );
    let forced = run(&[(0.0, reading(Some(30.0), ChannelQuality::Forced), good(45.0))]);
    let evaluation = evaluate_recorded_limits(&forced, &checks(0.0)).expect("the checks hold");
    assert_eq!(evaluation.status, RequirementStatus::Inconclusive);
    assert_eq!(
        evaluation.finding.as_deref(),
        Some("ValveMV has no good value at 0 s: forced.")
    );
}

#[test]
fn the_plant_input_is_checked_first_and_the_first_problem_in_time_decides() {
    let recorded = run(&[
        (0.0, good(30.0), good(45.0)),
        (1.0, reading(Some(30.0), ChannelQuality::Bad), good(45.0)),
        (2.0, good(30.0), good(150.0)),
    ]);
    let evaluation = evaluate_recorded_limits(&recorded, &checks(2.0)).expect("the checks hold");
    assert_eq!(evaluation.status, RequirementStatus::Inconclusive);
    assert_eq!(
        evaluation.finding.as_deref(),
        Some("ValveMV has no good value at 1 s: bad.")
    );
}

#[test]
fn a_check_of_a_signal_the_run_does_not_hold_is_refused() {
    let recorded = run(&[(0.0, good(30.0), good(45.0))]);
    let unknown = vec![bounds("Unknown", 0.0, 100.0, 0.0)];
    assert!(evaluate_recorded_limits(&recorded, &unknown).is_err());
}
