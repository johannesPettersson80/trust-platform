//! Tests of the analysis records (B10.2): a kept result is answered again while its loop revision is current, a record of
//! another revision is not answered, and a recording the study does not list is refused until the study lists it.

use serde_json::json;

use super::*;
use crate::analysis::declared::DeclaredControlAnalysis;
use crate::analysis::measured_frequency::MeasuredPoint;
use crate::analysis::measured_paths::{
    MeasuredPathsResult, PairingStatus, PathChannel, PathPair, PathPairing, SettledTrial,
    TimeWindow,
};
use crate::analysis::routh_interval::calculate_declared_routh;

const RECORDING: &str = "rec-00000000000000000000000000000001";

/// The shaft of control-studio-declared-analysis.test.ts: 1/(s² + s) with Kd fixed at 7 and Kp swept over 12, 16 and 20.
fn shaft() -> DeclaredControlAnalysis {
    crate::json::decode_value(json!({
        "plant": { "kind": "transfer-function", "numerator": [1.0], "denominator": [1.0, 1.0, 0.0], "delaySeconds": 0.0,
            "inputUnit": "N·m", "outputUnit": "rad", "revision": "shaft-1", "source": "provided", "validation": "unvalidated" },
        "controller": { "numerator": [{ "parameter": "Kd" }, { "parameter": "Kp" }], "denominator": [1.0],
            "parameters": { "Kp": { "value": 12.0, "minimum": 0.0, "maximum": 100.0, "unit": "N·m/rad" },
                "Kd": { "value": 7.0, "minimum": 7.0, "maximum": 7.0, "unit": "N·m·s/rad" } } },
        "rootLocus": { "parameter": "Kp", "values": [12.0, 16.0, 20.0], "selectedValue": 16.0 }
    }))
    .expect("the shaft")
}

fn saved_routh() -> SavedRouthAnalysis {
    let definition = shaft();
    SavedRouthAnalysis {
        selected_value: 16.0,
        result: calculate_declared_routh(&definition, Some(16.0)).expect("the reading"),
        definition,
    }
}

fn measured_result(recording_id: &str) -> MeasuredFrequencyResult {
    MeasuredFrequencyResult {
        recording_id: recording_id.to_string(),
        recording_label: String::new(),
        recording_revision: "rev-1".to_string(),
        sample_time_seconds: 0.1,
        frequencies: vec![1.0, 2.0],
        segment_count: 4,
        minimum_coherence: 0.8,
        startup_samples: 16,
        status: MeasuredStatus::Qualified,
        reason: None,
        points: vec![
            MeasuredPoint {
                frequency_rad_per_second: 1.0,
                magnitude: 2.0,
                magnitude_db: 6.0,
                phase_degrees: -10.0,
                coherence: 0.99,
            },
            MeasuredPoint {
                frequency_rad_per_second: 2.0,
                magnitude: 1.5,
                magnitude_db: 3.5,
                phase_degrees: -30.0,
                coherence: 0.98,
            },
        ],
        summary: vec!["Measured 2 frequency points.".to_string()],
        details: Vec::new(),
    }
}

#[test]
fn a_kept_reading_is_answered_again_while_its_loop_revision_is_current() {
    let mut study = Study::new("loop-a", "rev-1");
    let saved = saved_routh();
    keep(
        &mut study,
        "rev-1",
        KeptAnalysis::Routh(Box::new(saved.clone())),
    )
    .expect("the reading is kept");
    let records = current_records(&study, "rev-1", None);
    assert_eq!(records.routh.as_ref(), Some(&saved.result));
    assert!(records.root_locus.is_none());
    assert!(records.measured_frequency.is_none());
}

#[test]
fn a_record_of_another_loop_revision_is_not_answered() {
    let mut study = Study::new("loop-a", "rev-1");
    keep(
        &mut study,
        "rev-1",
        KeptAnalysis::Routh(Box::new(saved_routh())),
    )
    .expect("the reading is kept");
    assert!(current_records(&study, "rev-2", None).routh.is_none());
}

/// The measured MIMO paths of a study with two inputs and two outputs, on the recording `recording_id` (N4.5).
fn paths_result(recording_id: &str) -> MeasuredPathsResult {
    let channel = |id: &str, symbol: &str| PathChannel {
        id: id.to_string(),
        symbol: symbol.to_string(),
        unit: "degC".to_string(),
    };
    let window = |start_seconds: f64, end_seconds: f64| TimeWindow {
        start_seconds: Some(start_seconds),
        end_seconds: Some(end_seconds),
    };
    MeasuredPathsResult {
        recording_id: recording_id.to_string(),
        recording_revision: "rev-1".to_string(),
        inputs: vec![
            channel("heater-1", "P1.Heater1Pct"),
            channel("heater-2", "P1.Heater2Pct"),
        ],
        outputs: vec![
            channel("vessel-1", "P1.Vessel1C"),
            channel("vessel-2", "P1.Vessel2C"),
        ],
        trials: vec![
            SettledTrial {
                input_id: "heater-1".to_string(),
                baseline: window(0.0, 1.9),
                settled: window(27.1, 31.9),
            },
            SettledTrial {
                input_id: "heater-2".to_string(),
                baseline: window(57.1, 61.9),
                settled: window(87.1, 92.0),
            },
        ],
        sample_count: 670,
        gain_matrix: vec![vec![0.1380, 0.0607], vec![0.0611, 0.1376]],
        relative_gain_array: vec![vec![1.2426, -0.2426], vec![-0.2426, 1.2426]],
        decoupler: vec![vec![1.2426, -0.5464], vec![-0.5518, 1.2426]],
        gain_condition_number: 1.0,
        input_rank: 2,
        input_condition_number: 1.0,
        residual_norm: 0.0,
        pairing: PathPairing {
            status: PairingStatus::Named,
            pairs: vec![
                PathPair {
                    output_id: "vessel-1".to_string(),
                    input_id: "heater-1".to_string(),
                    relative_gain: 1.2426,
                },
                PathPair {
                    output_id: "vessel-2".to_string(),
                    input_id: "heater-2".to_string(),
                    relative_gain: 1.2426,
                },
            ],
            reason: None,
        },
        summary: vec!["670 samples from 2 independent input steps.".to_string()],
        details: Vec::new(),
    }
}

#[test]
fn measured_paths_are_kept_once_the_study_lists_their_recording() {
    let mut study = Study::new("loop-a", "rev-1");
    let result = paths_result(RECORDING);
    assert!(keep(
        &mut study,
        "rev-1",
        KeptAnalysis::MeasuredPaths(Box::new(result.clone()))
    )
    .is_err());
    assert!(study.sections.is_empty(), "a refused record keeps nothing");
    study.recordings.push(RECORDING.to_string());
    keep(
        &mut study,
        "rev-1",
        KeptAnalysis::MeasuredPaths(Box::new(result.clone())),
    )
    .expect("the record is kept once the study lists its recording");
    assert_eq!(
        current_records(&study, "rev-1", None).measured_paths,
        Some(result)
    );
    assert!(
        current_records(&study, "rev-2", None)
            .measured_paths
            .is_none(),
        "a record of another loop revision is not answered"
    );
}

#[test]
fn a_recording_the_study_does_not_list_is_refused_until_the_study_lists_it() {
    let mut study = Study::new("loop-a", "rev-1");
    let result = measured_result(RECORDING);
    assert!(keep(
        &mut study,
        "rev-1",
        KeptAnalysis::MeasuredFrequency(Box::new(result.clone()))
    )
    .is_err());
    assert!(study.sections.is_empty(), "a refused record keeps nothing");
    assert!(current_records(&study, "rev-1", None)
        .measured_frequency
        .is_none());
    study.recordings.push(RECORDING.to_string());
    keep(
        &mut study,
        "rev-1",
        KeptAnalysis::MeasuredFrequency(Box::new(result.clone())),
    )
    .expect("the record is kept once the study lists its recording");
    assert_eq!(
        current_records(&study, "rev-1", None).measured_frequency,
        Some(result)
    );
}
