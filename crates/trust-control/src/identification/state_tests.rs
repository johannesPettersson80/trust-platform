//! Tests of Identify's saved state. The `TypeScript` tests (`control-studio-identification-state.test.ts`) check the
//! decoder through the workflow's save and restore; those are ported in `workflow_tests.rs`. The cases here pin each
//! rule of the decoder, the saved form of a sample, the identity of an imported recording and the original recording of
//! a selection on their own, without a fit.

use serde_json::json;

use super::*;
use crate::identification::form_draft::IDENTIFICATION_FORM_KEYS;
use crate::recording::fixtures::{binding, loop_definition, run, sample};
use crate::signals::Quality;

/// A saved input of two samples of recording `recording-1`, checked with the error limit 0.15.
fn input_json() -> Value {
    json!({
        "estimation": [
            { "timeSeconds": 0.0, "input": 1.0, "output": 3.0, "quality": "good",
              "inputUnit": "%", "outputUnit": "degC" },
            { "timeSeconds": 0.1, "input": 2.0, "output": 3.5, "quality": "good",
              "inputUnit": "%", "outputUnit": "degC" }
        ],
        "excluded": [],
        "source": "Simulation",
        "requestedKind": "first_order",
        "maximumNormalizedError": 0.15,
        "estimationRecordingId": "recording-1"
    })
}

/// A saved state at `phase` with the current input of [`input_json`].
fn state_json(phase: &str) -> Value {
    json!({
        "schemaVersion": 1,
        "maximumNormalizedError": 0.15,
        "phase": phase,
        "current": input_json()
    })
}

/// A form draft with every field of the held-step settings.
fn form_json() -> Value {
    let values: serde_json::Map<String, Value> = IDENTIFICATION_FORM_KEYS
        .iter()
        .map(|key| ((*key).to_string(), json!("5")))
        .collect();
    json!({
        "schemaVersion": 1, "model": "first_order", "heldStep": false, "values": values,
        "excludedStart": "", "excludedEnd": ""
    })
}

/// The held-step settings of the `TypeScript` state tests.
fn settings_json() -> Value {
    json!({
        "baselineDurationSeconds": 5.0, "finalDurationSeconds": 5.0, "maximumWindowRange": 0.1,
        "maximumWindowSlopePerSecond": 0.01, "measurementBound": 0.05, "onsetThreshold": 0.15, "onsetSamples": 3,
        "delay": { "minimum": 0.0, "maximum": 2.0, "step": 0.2 },
        "timeConstant": { "minimum": 4.0, "maximum": 6.0, "step": 0.2 },
        "baselineStep": 0.01, "gainStep": 0.001, "timestampShiftSeconds": 0.0
    })
}

fn decodes(value: &Value) -> bool {
    decode_identification_state(value, &loop_definition()).is_some()
}

#[test]
fn a_qualified_state_decodes_and_encodes_as_it_was_saved() {
    let value = state_json("qualified");
    let state = decode_identification_state(&value, &loop_definition()).expect("the state decodes");
    assert_eq!(state.phase, IdentificationPhase::Qualified);
    let current = state.current.as_ref().expect("the current input is kept");
    assert_eq!(
        current.estimation_recording_id.as_deref(),
        Some("recording-1")
    );
    assert_eq!(current.estimation[1].input, Some(2.0));
    assert_eq!(
        serde_json::to_value(&state).expect("the state encodes"),
        value
    );
}

#[test]
fn the_stage_needs_the_inputs_it_stands_for() {
    let mut editing = state_json("editing");
    assert!(!decodes(&editing), "an editing state has a form");
    editing["form"] = form_json();
    assert!(decodes(&editing));
    let mut estimation_only = editing;
    estimation_only["estimation"] = input_json();
    estimation_only
        .as_object_mut()
        .expect("an object")
        .remove("current");
    assert!(
        !decodes(&estimation_only),
        "an editing estimation needs a current input beside it"
    );

    for phase in ["preview", "validation-qualified", "review", "accepted"] {
        assert!(!decodes(&state_json(phase)), "{phase} needs an estimation");
        let mut with_estimation = state_json(phase);
        with_estimation["estimation"] = input_json();
        assert!(
            decodes(&with_estimation),
            "{phase} with an estimation decodes"
        );
    }
    let mut qualified = state_json("qualified");
    qualified["estimation"] = input_json();
    assert!(
        !decodes(&qualified),
        "a qualified estimation has no estimation yet"
    );

    let mut preview = state_json("preview");
    preview["estimation"] = input_json();
    preview["estimation"]["estimation"][1]["output"] = json!(3.6);
    assert!(
        !decodes(&preview),
        "a preview's current input is its estimation"
    );
    preview
        .as_object_mut()
        .expect("an object")
        .remove("current");
    assert!(!decodes(&preview), "a preview has a current input");
    assert!(
        !decodes(&state_json("unknown")),
        "the stage is one of the nine"
    );
}

#[test]
fn every_saved_input_has_the_states_error_limit() {
    let mut other = state_json("qualified");
    other["current"]["maximumNormalizedError"] = json!(0.2);
    assert!(!decodes(&other));
    let mut missing = state_json("qualified");
    missing["current"]
        .as_object_mut()
        .expect("an object")
        .remove("maximumNormalizedError");
    assert!(!decodes(&missing));
    for limit in [0.0, -0.15] {
        let mut state = state_json("qualified");
        state["maximumNormalizedError"] = json!(limit);
        state["current"]["maximumNormalizedError"] = json!(limit);
        assert!(!decodes(&state), "the error limit is positive");
    }
    let mut null_limit = state_json("qualified");
    null_limit["maximumNormalizedError"] = json!(null);
    assert!(!decodes(&null_limit));
    let mut version = state_json("qualified");
    version["schemaVersion"] = json!(2);
    assert!(!decodes(&version));
}

#[test]
fn a_sensor_lag_structure_choice_is_refused_with_its_comparison_parked() {
    let mut preview = state_json("preview");
    preview["estimation"] = input_json();
    assert!(decodes(&preview));
    preview["structureChoice"] = json!("first_order");
    assert!(
        !decodes(&preview),
        "the sensor-lag comparison is parked with stage V7"
    );
}

#[test]
fn unknown_and_null_keys_are_refused_while_null_numbers_read_as_missing() {
    for (key, value) in [
        ("declaredProcess", json!("integrating")),
        ("closedLoop", json!(false)),
        ("validationRecordingId", json!("validation")),
        ("validation", json!([])),
        ("estimationRecordingId", json!(null)),
        ("stepResponseSettings", json!(null)),
    ] {
        let mut state = state_json("qualified");
        state["current"][key] = value;
        assert!(!decodes(&state), "the saved input refuses {key}");
    }
    let mut null_current = state_json("qualified");
    null_current["current"] = json!(null);
    assert!(!decodes(&null_current));
    let mut unknown = state_json("qualified");
    unknown["chart"] = json!({});
    assert!(!decodes(&unknown));

    let mut null_unit = state_json("qualified");
    null_unit["current"]["estimation"][0]["inputUnit"] = json!(null);
    assert!(!decodes(&null_unit));
    let mut extra = state_json("qualified");
    extra["current"]["estimation"][0]["scan"] = json!(3);
    assert!(!decodes(&extra));

    let mut null_time = state_json("qualified");
    null_time["current"]["estimation"][0]["timeSeconds"] = json!(null);
    let state = decode_identification_state(&null_time, &loop_definition())
        .expect("null is a missing number");
    assert_eq!(
        state.current.expect("the input is kept").estimation[0].time_seconds,
        None
    );
    let mut no_time = state_json("qualified");
    no_time["current"]["estimation"][0]
        .as_object_mut()
        .expect("an object")
        .remove("timeSeconds");
    assert!(
        !decodes(&no_time),
        "the time is present, as a number or null"
    );
}

#[test]
fn units_recording_identifiers_and_excluded_intervals_are_bounded() {
    let mut unit = state_json("qualified");
    unit["current"]["estimation"][0]["outputUnit"] = json!("é".repeat(256));
    assert!(decodes(&unit), "256 UTF-16 code units fit");
    unit["current"]["estimation"][0]["outputUnit"] = json!("u".repeat(257));
    assert!(!decodes(&unit));

    let mut id = state_json("qualified");
    id["current"]["estimationRecordingId"] = json!("r".repeat(1024));
    assert!(decodes(&id));
    id["current"]["estimationRecordingId"] = json!("r".repeat(1025));
    assert!(!decodes(&id));
    id["current"]["estimationRecordingId"] = json!("");
    assert!(!decodes(&id));

    let mut interval = state_json("qualified");
    interval["current"]["excluded"] = json!([{ "startSeconds": 1.0, "endSeconds": 1.0 }]);
    assert!(decodes(&interval));
    interval["current"]["excluded"] = json!([{ "startSeconds": 2.0, "endSeconds": 1.0 }]);
    assert!(
        !decodes(&interval),
        "an interval does not end before it starts"
    );
}

#[test]
fn first_order_reference_settings_belong_to_a_two_lag_request() {
    let mut held_step = state_json("qualified");
    held_step["current"]["stepResponseSettings"] = settings_json();
    assert!(decodes(&held_step));
    held_step["current"]["firstOrderStepResponseSettings"] = settings_json();
    assert!(
        !decodes(&held_step),
        "only a two-lag request keeps first-order reference settings"
    );
    let mut sopdt = held_step;
    sopdt["current"]["requestedKind"] = json!("sopdt");
    assert!(decodes(&sopdt));
    sopdt["current"]["stepResponseSettings"]["measurementBound"] = json!(0.0);
    assert!(!decodes(&sopdt), "the step-response settings are valid");
}

/// The fixture loop's recording of three samples, 100 ms apart: the measurement and the manipulated variable.
fn recorded_run() -> RecordedRun {
    run(
        vec![
            binding("processVariable", "%IL0", "LREAL"),
            binding("manipulatedVariable", "%QL0", "LREAL"),
        ],
        vec![
            sample(1, Some(0), &[Some(3.0), Some(1.0)], &[Quality::Good; 2]),
            sample(
                2,
                Some(100_000_000),
                &[Some(3.0), Some(2.0)],
                &[Quality::Good; 2],
            ),
            sample(
                3,
                Some(200_000_000),
                &[Some(3.5), Some(2.0)],
                &[Quality::Good; 2],
            ),
        ],
    )
}

/// A qualified state whose input keeps `recording` and the samples it yields for the fixture loop.
fn state_with_recording(recording: &IdentificationRecording, id: &str) -> Value {
    let samples = retained_identification_samples_from_runtime_recording(
        recording.kind,
        &recording.run,
        &loop_definition(),
    )
    .expect("the run yields samples");
    let mut state = state_json("qualified");
    state["current"]["estimation"] = serde_json::to_value(samples).expect("the samples encode");
    state["current"]["estimationRecordingId"] = json!(id);
    state["current"]["recording"] = serde_json::to_value(recording).expect("the recording encodes");
    state
}

#[test]
fn a_retained_recording_must_yield_the_saved_samples_under_its_own_identity() {
    let recording = identification_original_recording(
        Some(&recorded_run()),
        ControlExperimentKind::MvStep,
        None,
    )
    .expect("a run gives a recording");
    let state = state_with_recording(&recording, "rec-fixture");
    assert!(decodes(&state));

    let mut other_id = state.clone();
    other_id["current"]["estimationRecordingId"] = json!("rec-other");
    assert!(
        !decodes(&other_id),
        "the input names the recording it came from"
    );
    let mut changed = state.clone();
    changed["current"]["recording"]["run"]["samples"][1]["values"][0] = json!(13.0);
    assert!(
        !decodes(&changed),
        "the recording yields exactly the saved samples"
    );
    let mut setpoint = state;
    setpoint["current"]["recording"]["kind"] = json!("setpoint_step");
    assert!(
        !decodes(&setpoint),
        "the kind selects the plant input the samples were taken from"
    );

    let provenance = RecordingProvenance {
        project_id: "project".to_string(),
        result_id: "result-1".to_string(),
        result_revision: 3,
    };
    let imported = identification_original_recording(
        Some(&recorded_run()),
        ControlExperimentKind::MvStep,
        Some(provenance.clone()),
    )
    .expect("a run gives a recording");
    assert!(decodes(&state_with_recording(
        &imported,
        &imported_recording_id(&provenance)
    )));
    assert!(
        !decodes(&state_with_recording(&imported, "rec-fixture")),
        "an imported recording is known by its saved result"
    );
}

#[test]
fn an_imported_recording_is_identified_by_its_saved_result() {
    let provenance = RecordingProvenance {
        project_id: "project".to_string(),
        result_id: "result-1".to_string(),
        result_revision: 3,
    };
    assert_eq!(
        imported_recording_id(&provenance),
        r#"saved-recording:["project","result-1",3]"#
    );
    let quoted = RecordingProvenance {
        project_id: "a\"b".to_string(),
        result_id: "r".to_string(),
        result_revision: 0,
    };
    assert_eq!(
        imported_recording_id(&quoted),
        r#"saved-recording:["a\"b","r",0]"#
    );
}

#[test]
fn the_original_recording_of_a_selection_has_no_selected_interval() {
    assert_eq!(
        identification_original_recording(None, ControlExperimentKind::Observation, None),
        None
    );
    let recorded = recorded_run();
    let recording = identification_original_recording(
        Some(&recorded),
        ControlExperimentKind::ProjectSequence,
        None,
    )
    .expect("a run gives a recording");
    assert_eq!(recording.kind, ControlExperimentKind::ProjectSequence);
    assert_eq!(recording.run, recorded);
    assert_eq!(
        recording.selected_intervals,
        [] as [crate::identification::data::ExcludedInterval; 0]
    );
    let value = serde_json::to_value(&recording).expect("the recording encodes");
    assert_eq!(value["kind"], json!("project_sequence"));
    assert_eq!(value["selectedIntervals"], json!([]));
    assert_eq!(value["run"]["recording_id"], json!("rec-fixture"));
    assert!(value.get("provenance").is_none());
}

#[test]
fn samples_save_missing_numbers_as_null_and_restore_them_as_nan() {
    let samples = vec![
        IdentificationSample {
            time_seconds: 0.0,
            input: f64::NAN,
            output: f64::INFINITY,
            quality: SampleQuality::Bad,
            input_unit: Some("%".to_string()),
            output_unit: None,
            clipped: Some(true),
            saturated: None,
        },
        IdentificationSample {
            time_seconds: 0.1,
            input: 2.0,
            output: 3.5,
            quality: SampleQuality::Good,
            input_unit: Some("%".to_string()),
            output_unit: Some("degC".to_string()),
            clipped: None,
            saturated: Some(true),
        },
    ];
    let input = IdentificationReviewInput {
        recording: None,
        step_response_settings: None,
        first_order_step_response_settings: None,
        maximum_normalized_error: Some(0.15),
        estimation: samples.clone(),
        validation: Some(samples.clone()),
        estimation_recording_id: Some("recording-1".to_string()),
        validation_recording_id: Some("recording-2".to_string()),
        excluded: Vec::new(),
        source: DataSource::Physical,
        requested_kind: IdentificationModelSelection::Automatic,
        declared_process: None,
        closed_loop: None,
    };
    let saved = save_identification_input(&input);
    assert_eq!(saved.estimation[0].time_seconds, Some(0.0));
    assert_eq!(saved.estimation[0].input, None);
    assert_eq!(saved.estimation[0].output, None);
    let value = serde_json::to_value(&saved).expect("the input encodes");
    assert_eq!(value["estimation"][0]["input"], json!(null));
    assert_eq!(value["estimation"][0]["output"], json!(null));
    assert!(value.get("validation").is_none() && value.get("validationRecordingId").is_none());

    let restored = restore_identification_input(&saved);
    assert!(restored.estimation[0].input.is_nan() && restored.estimation[0].output.is_nan());
    assert_eq!(restored.estimation[1], samples[1]);
    assert_eq!(restored.validation, None);
    assert_eq!(restored.validation_recording_id, None);
    assert_eq!(
        restored.estimation_recording_id.as_deref(),
        Some("recording-1")
    );
    assert_eq!(restored.source, DataSource::Physical);
    assert!(
        same_samples(&restored.estimation, &samples),
        "NaN and infinity both print as null"
    );
    assert!(!same_samples(&restored.estimation[..1], &samples));
}
