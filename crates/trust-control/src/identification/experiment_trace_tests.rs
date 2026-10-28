//! Tests of the traces of an identification experiment (`identificationExperiment.ts`).

#![allow(clippy::float_cmp)] // The traces hold the recorded values exactly.

use serde_json::json;

use super::{
    decode_identification_experiment, experiment_from_run, experiment_from_samples,
    ExperimentSignal, ExperimentSignals,
};
use crate::identification::connected_record::RuntimeVerificationResult;
use crate::identification::data::IdentificationSample;
use crate::identification::independent_validation::RequirementStatus;
use crate::identification::record_fixtures::{setpoint_record, Row, Verdict};
use crate::loop_definition::LoopDefinition;
use trust_control_math::identify::SampleQuality;

const CHANNELS: &[(&str, &str)] = &[("MV", "%"), ("PV", "degC")];

fn loop_definition() -> LoopDefinition {
    let mut loop_definition = crate::recording::fixtures::loop_definition();
    loop_definition.signals.process_variable.symbol = "PV".to_owned();
    loop_definition.signals.process_variable.unit = "degC".to_owned();
    loop_definition.signals.manipulated_variable.symbol = "MV".to_owned();
    loop_definition.signals.manipulated_variable.unit = "%".to_owned();
    loop_definition
}

fn sample(time: f64, input: f64, output: f64, quality: SampleQuality) -> IdentificationSample {
    IdentificationSample {
        time_seconds: time,
        input,
        output,
        quality,
        input_unit: Some("%".to_owned()),
        output_unit: Some("degC".to_owned()),
        clipped: None,
        saturated: None,
    }
}

#[test]
fn samples_trace_only_good_finite_values() {
    let samples = [
        sample(0.0, 50.0, 20.0, SampleQuality::Good),
        sample(0.1, 50.0, 21.0, SampleQuality::Bad),
        sample(0.2, 60.0, f64::NAN, SampleQuality::Good),
    ];
    let experiment = experiment_from_samples(&loop_definition(), &samples);
    assert_eq!(experiment.recording_id, None);
    assert_eq!(experiment.inputs.len(), 1);
    assert_eq!(experiment.inputs[0].id, "MV");
    assert_eq!(experiment.inputs[0].unit, "%");
    let inputs: Vec<Option<f64>> = experiment.inputs[0]
        .points
        .iter()
        .map(|point| point.value)
        .collect();
    assert_eq!(inputs, vec![Some(50.0), None, Some(60.0)]);
    let outputs: Vec<Option<f64>> = experiment.outputs[0]
        .points
        .iter()
        .map(|point| point.value)
        .collect();
    assert_eq!(outputs, vec![Some(20.0), None, None]);
    assert_eq!(experiment.outputs[0].id, "PV");
}

#[test]
fn a_recording_is_traced_with_its_recorded_values_and_the_loop_signals_by_default() {
    let rows: Vec<Row> = vec![
        (0.0, 0, vec![Some(50.0), Some(20.0)]),
        (0.1, 1, vec![Some(50.0), None]),
    ];
    let record: RuntimeVerificationResult<Verdict> =
        setpoint_record("experiment", CHANNELS, &rows, RequirementStatus::Pass);
    let experiment = experiment_from_run(&loop_definition(), &record.run, None)
        .expect("the signals are in the run");
    assert_eq!(experiment.recording_id.as_deref(), Some("experiment"));
    assert_eq!(experiment.outputs[0].points[0].value, Some(20.0));
    assert_eq!(
        experiment.outputs[0].points[1].value, None,
        "a missing value is not traced"
    );
    assert_eq!(experiment.inputs[0].points[1].time_seconds, 0.1);
}

#[test]
fn a_selected_signal_must_be_in_the_recording_with_its_unit() {
    let rows: Vec<Row> = vec![(0.0, 0, vec![Some(50.0), Some(20.0)])];
    let record = setpoint_record("experiment", CHANNELS, &rows, RequirementStatus::Pass);
    let wrong_unit = ExperimentSignals {
        inputs: vec![ExperimentSignal {
            id: "MV".to_owned(),
            unit: "degC".to_owned(),
        }],
        outputs: vec![ExperimentSignal {
            id: "PV".to_owned(),
            unit: "degC".to_owned(),
        }],
    };
    let refused = experiment_from_run(&loop_definition(), &record.run, Some(&wrong_unit));
    assert_eq!(
        refused.err().map(|refusal| refusal.0),
        Some("The experiment recording does not contain its selected signal and unit.".to_owned())
    );
}

#[test]
fn stored_traces_decode_only_when_they_are_well_formed() {
    let valid = json!({
        "recordingId": "experiment",
        "inputs": [{ "id": "MV", "unit": "%", "points": [{ "timeSeconds": 0.0, "value": 50.0 }, { "timeSeconds": 0.1, "value": null }] }],
        "outputs": [{ "id": "PV", "unit": "degC", "points": [{ "timeSeconds": 0.0, "value": 20.0 }] }]
    });
    assert!(decode_identification_experiment(&valid).is_some());
    let unordered = json!({
        "inputs": [{ "id": "MV", "unit": "%", "points": [{ "timeSeconds": 0.2, "value": 50.0 }, { "timeSeconds": 0.1, "value": 50.0 }] }],
        "outputs": [{ "id": "PV", "unit": "degC", "points": [] }]
    });
    assert!(
        decode_identification_experiment(&unordered).is_none(),
        "times must increase"
    );
    let no_inputs =
        json!({ "inputs": [], "outputs": [{ "id": "PV", "unit": "degC", "points": [] }] });
    assert!(
        decode_identification_experiment(&no_inputs).is_none(),
        "a list needs one trace"
    );
    let repeated = json!({
        "inputs": [{ "id": "MV", "unit": "%", "points": [] }, { "id": "MV", "unit": "%", "points": [] }],
        "outputs": [{ "id": "PV", "unit": "degC", "points": [] }]
    });
    assert!(
        decode_identification_experiment(&repeated).is_none(),
        "identifiers are distinct"
    );
    let empty_recording = json!({
        "recordingId": "",
        "inputs": [{ "id": "MV", "unit": "%", "points": [] }],
        "outputs": [{ "id": "PV", "unit": "degC", "points": [] }]
    });
    assert!(
        decode_identification_experiment(&empty_recording).is_none(),
        "a recording identifier is not blank"
    );
}
