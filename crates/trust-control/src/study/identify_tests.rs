//! Tests of the Identify section (specification 35, A8.1, A9, B10.2 and B10.4; Identify page specification, "Single
//! experiment and provisional model"). The model and its presentation are those of
//! `control-studio-engineer-study-persistence.test.ts` ("retains the model-bound measured comparison through study
//! restore"); the rules of the study files are those of `control-studio-study-store.test.ts`.

use serde_json::{json, Value};
use trust_control_math::identify::LowOrderModelKind;

use super::*;
use crate::identification::evidence::{ModelFitEvidence, ModelFitPoint, OperatingRange};
use crate::identification::identify_result::tests::plant_loop;
use crate::identification::independent_validation::RequirementStatus;
use crate::identification::model_comparison::ComparisonPresentation;
use crate::identification::plant::IdentifiedModel;
use crate::identification::presentation::identified_model_presentation;
use crate::identification::record_fixtures::setpoint_record;
use crate::limits::STUDY_STORAGE_LIMIT_BYTES;
use crate::model::{decode_general_model, ModelIdentity};
use crate::recording::evidence::{recording_evidence, UNCONFIRMED_START_RECOVERED};
use crate::recording::fixtures::{binding, run, sample};
use crate::signals::Quality;
use crate::study::{canonical_json, Study};

/// The model of the `TypeScript` test: `1 / (s + 1)` from a command in V to a position in m, fitted from the evidence given.
fn fitted_model(validation: &str, evidence: &[&str]) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:provided:1", "source": "fitted",
        "validation": validation, "evidence": evidence, "delaySeconds": 0.0,
        "inputs": [{ "id": "command", "unit": "V", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
        "outputs": [{ "id": "position", "unit": "m", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
        "numerator": [1.0], "denominator": [1.0, 1.0]
    }))
    .expect("a fitted model")
}

/// The model with its identity changed.
fn edited(mut model: GeneralModel, change: impl FnOnce(&mut ModelIdentity)) -> GeneralModel {
    match &mut model {
        GeneralModel::TransferFunction { identity, .. }
        | GeneralModel::ContinuousStateSpace { identity, .. }
        | GeneralModel::DiscreteStateSpace { identity, .. } => change(identity),
    }
    model
}

/// The presentation of the `TypeScript` test: a first-order fit (gain 1, time constant 1, error 0.1) with the points
/// (0, 0, 0, 0) and (2, 1, 0.9, 0.1), presented in V and m, the units of the fitted model.
fn comparison_of() -> ComparisonPresentation {
    let identified = IdentifiedModel {
        kind: LowOrderModelKind::FirstOrder,
        gain: 1.0,
        time_constant_seconds: Some(1.0),
        second_time_constant_seconds: None,
        dead_time_seconds: None,
        normalized_error: 0.1,
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    };
    let fit = ModelFitEvidence {
        points: vec![
            ModelFitPoint {
                time_seconds: 0.0,
                measured: 0.0,
                predicted: 0.0,
                residual: 0.0,
            },
            ModelFitPoint {
                time_seconds: 2.0,
                measured: 1.0,
                predicted: 0.9,
                residual: 0.1,
            },
        ],
        root_mean_square_error: 0.1,
        output_unit: "m".to_owned(),
        stable: true,
        final_change_fraction: None,
        delay_seconds: 0.0,
        operating_range: OperatingRange {
            input_minimum: 0.0,
            input_maximum: 1.0,
            output_minimum: 0.0,
            output_maximum: 1.0,
        },
        warnings: Vec::new(),
        step_response: None,
        step_response_measurement: None,
    };
    ComparisonPresentation {
        presentation: identified_model_presentation(&identified, &fit, "V", "m"),
        experiment: None,
        output_id: None,
        input_channels: None,
    }
}

/// A complete recording of two consecutive samples; `first` makes recordings differ.
fn recording_file(source: EvidenceSource, first: f64) -> RecordingFile {
    let recorded = run(
        vec![binding("processVariable", "%IL0", "LREAL")],
        vec![
            sample(1, Some(0), &[Some(first)], &[Quality::Good]),
            sample(2, Some(10_000_000), &[Some(first + 0.5)], &[Quality::Good]),
        ],
    );
    let evidence = recording_evidence(&recorded, None);
    RecordingFile::new("loop:tank", source, recorded, evidence)
}

fn id_of(file: &RecordingFile) -> String {
    file.id().expect("identified")
}

fn signals() -> IdentifySignals {
    IdentifySignals {
        inputs: vec!["Plant.Command".into()],
        outputs: vec!["Plant.Position".into()],
        setpoint: Some("Plant.Target".into()),
    }
}

/// The provisional result of a live step on a simulated plant, fitted on `estimation`.
fn provisional(estimation: &RecordingFile) -> IdentifySection {
    let model = fitted_model("unvalidated", &["sha256:recording-fingerprint"]);
    let comparison = comparison_of();
    IdentifySection::new(
        IdentifyMethod::Live(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        signals(),
        &id_of(estimation),
        None,
        model,
        comparison,
    )
}

/// A validated result: fitted on `estimation` and judged on the distinct `validation` (N3.4).
fn validated(estimation: &RecordingFile, validation: &RecordingFile) -> IdentifySection {
    let model = fitted_model("validated", &["sha256:estimation", "sha256:validation"]);
    let comparison = comparison_of();
    IdentifySection::new(
        IdentifyMethod::Opened(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        signals(),
        &id_of(estimation),
        Some(&id_of(validation)),
        model,
        comparison,
    )
}

fn refusal(section: &IdentifySection) -> String {
    section.encode().expect_err("refused").message
}

#[test]
fn a_provisional_result_round_trips_through_its_document_and_the_study() {
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let section = provisional(&estimation);
    assert!(
        section.provisional,
        "a fit from one recording is provisional"
    );
    let text = section.encode().expect("encoded");
    let decoded = IdentifySection::decode(&text).expect("decoded");
    assert_eq!(decoded, section);
    assert_eq!(
        decoded.encode().expect("again"),
        text,
        "the same section gives the same canonical JSON"
    );

    let mut study = Study::new("loop:tank", "sha256:loop-1");
    study.set_section(&section).expect("kept");
    assert_eq!(
        study.recordings,
        vec![id_of(&estimation)],
        "the recording the result came from is listed"
    );
    let document = study.encode().expect("encoded");
    let reopened = Study::decode(&document, "loop:tank").expect("decoded");
    let read = reopened
        .section::<IdentifySection>()
        .expect("read")
        .expect("the section");
    assert_eq!(read.encode().expect("again"), text);
}

#[test]
fn the_section_is_stored_under_the_words_of_the_engineer_study() {
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let value = serde_json::to_value(provisional(&estimation)).expect("json");
    assert_eq!(value["method"], json!({ "live": "mv_step" }));
    assert_eq!(value["source"], json!("simulated-plant"));
    assert_eq!(
        value["signals"],
        json!({ "inputs": ["Plant.Command"], "outputs": ["Plant.Position"], "setpoint": "Plant.Target" })
    );
    assert_eq!(value["provisional"], json!(true));
    assert_eq!(value["estimationRecordingId"], json!(id_of(&estimation)));
    assert_eq!(
        value.get("validationRecordingId"),
        None,
        "a provisional result names no validation recording"
    );
    assert_eq!(
        value["selectedModel"]["revision"],
        json!("model:provided:1")
    );
    assert_eq!(value["presentation"]["units"]["output"], json!("m"));
    let opened =
        serde_json::to_value(IdentifyMethod::Opened(ControlExperimentKind::Relay)).expect("json");
    assert_eq!(opened, json!({ "opened": "relay" }));
}

#[test]
fn a_model_judged_on_a_distinct_recording_is_validated_and_not_provisional() {
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let validation = recording_file(EvidenceSource::SimulatedPlant, 30.0);
    assert_ne!(id_of(&estimation), id_of(&validation));
    let section = validated(&estimation, &validation);
    assert!(!section.provisional);
    let text = section.encode().expect("encoded");
    assert_eq!(IdentifySection::decode(&text).expect("decoded"), section);

    let mut study = Study::new("loop:tank", "sha256:loop-1");
    study.set_section(&section).expect("kept");
    assert_eq!(
        study.recordings,
        vec![id_of(&estimation), id_of(&validation)],
        "both recordings are listed"
    );
}

#[test]
fn a_fit_from_one_recording_is_never_stored_as_validated_evidence() {
    // Identify page specification, "Single experiment and provisional model".
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let validation = recording_file(EvidenceSource::SimulatedPlant, 30.0);

    let mut same_record = provisional(&estimation);
    same_record.selected_model = fitted_model("validated", &["sha256:recording-fingerprint"]);
    same_record.presentation = comparison_of();
    let message = refusal(&same_record);
    assert!(message.contains("never validated evidence"), "{message}");

    // Naming the estimation recording as its own validation recording does not make it independent (N3.4).
    let mut itself = validated(&estimation, &validation);
    itself.validation_recording_id = Some(id_of(&estimation));
    let message = refusal(&itself);
    assert!(message.contains("another recording"), "{message}");

    let mut unnamed = validated(&estimation, &validation);
    unnamed.validation_recording_id = None;
    let message = refusal(&unnamed);
    assert!(
        message.contains("names the recording it was validated on"),
        "{message}"
    );

    let mut named = provisional(&estimation);
    named.validation_recording_id = Some(id_of(&validation));
    let message = refusal(&named);
    assert!(
        message.contains("names no validation recording"),
        "{message}"
    );

    let mut unvalidated = validated(&estimation, &validation);
    unvalidated.selected_model = fitted_model("unvalidated", &["sha256:estimation"]);
    unvalidated.presentation = comparison_of();
    let message = refusal(&unvalidated);
    assert!(message.contains("must be validated"), "{message}");

    let mut traversal = validated(&estimation, &validation);
    traversal.validation_recording_id = Some("../study".into());
    let message = refusal(&traversal);
    assert!(message.contains("not a recording identifier"), "{message}");
}

#[test]
fn the_signals_are_named_once_and_match_the_models_channels() {
    type Edit = fn(&mut IdentifySignals);
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let cases: [(Edit, &str); 5] = [
        (|signals| signals.inputs.clear(), "names 0 plant input"),
        (
            |signals| signals.outputs.push("Plant.Extra".into()),
            "2 measured output",
        ),
        (
            |signals| signals.inputs[0] = "  ".into(),
            "blank or named twice",
        ),
        (
            |signals| signals.setpoint = Some("Plant.Command".into()),
            "named twice",
        ),
        (
            |signals| signals.outputs[0] = "Plant.Command".into(),
            "named twice",
        ),
    ];
    for (change, expected) in cases {
        let mut section = provisional(&estimation);
        change(&mut section.signals);
        let message = refusal(&section);
        assert!(message.contains(expected), "{expected}: {message}");
    }
    let mut without_setpoint = provisional(&estimation);
    without_setpoint.signals.setpoint = None;
    without_setpoint
        .encode()
        .expect("only a closed-loop identification names a setpoint");
}

#[test]
fn the_model_is_fitted_with_evidence_and_its_presentation_is_bound_to_it() {
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);

    let mut provided = provisional(&estimation);
    provided.selected_model = edited(provided.selected_model, |identity| {
        identity.source = ModelSource::Provided;
    });
    assert!(refusal(&provided).contains("not fitted from a recording"));

    let mut without_evidence = provisional(&estimation);
    without_evidence.selected_model = edited(without_evidence.selected_model, |identity| {
        identity.evidence.clear();
    });
    assert!(refusal(&without_evidence).contains("no evidence"));

    let mut blank = provisional(&estimation);
    blank.selected_model = edited(blank.selected_model, |identity| {
        identity.revision = " ".into();
    });
    assert!(refusal(&blank).contains("model contract"));

    // The presentation must describe its own model: a negative delay is refused ...
    let mut delayed = provisional(&estimation);
    delayed.presentation.presentation.delay_seconds = -1.0;
    assert!(refusal(&delayed).contains("does not describe the model"));

    // ... and so is a presentation whose output unit is not the model's.
    let mut wrong_unit = provisional(&estimation);
    wrong_unit.presentation.presentation.units.output = "mm".into();
    assert!(refusal(&wrong_unit).contains("does not describe the model"));
}

#[test]
fn a_document_with_an_unknown_or_a_missing_field_or_word_is_refused_whole() {
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let value = serde_json::to_value(provisional(&estimation)).expect("json");
    IdentifySection::decode(&canonical_json(&value)).expect("the section as it is stored");

    let mut extra = value.clone();
    extra["approved"] = json!(true);
    assert!(
        IdentifySection::decode(&canonical_json(&extra)).is_err(),
        "an approval is not part of the section"
    );
    for key in [
        "method",
        "source",
        "signals",
        "estimationRecordingId",
        "provisional",
        "selectedModel",
        "presentation",
    ] {
        let mut missing: Value = value.clone();
        missing.as_object_mut().expect("an object").remove(key);
        assert!(
            IdentifySection::decode(&canonical_json(&missing)).is_err(),
            "{key} is required"
        );
    }
    let mut method = value.clone();
    method["method"] = json!({ "live": "teleport" });
    assert!(IdentifySection::decode(&canonical_json(&method)).is_err());
    let mut source = value;
    source["source"] = json!("opened-recording");
    assert!(IdentifySection::decode(&canonical_json(&source)).is_err());
}

#[test]
fn the_recording_files_are_the_ones_named_of_one_source_and_complete() {
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let validation = recording_file(EvidenceSource::SimulatedPlant, 30.0);
    let section = validated(&estimation, &validation);
    section
        .check_recordings(&estimation, Some(&validation))
        .expect("the files the section names");
    assert!(
        section
            .check_recordings(&validation, Some(&estimation))
            .is_err(),
        "the files are swapped"
    );
    assert!(
        section.check_recordings(&estimation, None).is_err(),
        "the validation recording is missing"
    );
    let single = provisional(&estimation);
    assert!(
        single
            .check_recordings(&estimation, Some(&validation))
            .is_err(),
        "a provisional result names no validation recording"
    );

    // Simulated evidence is never presented as physical (A9).
    let mut physical = provisional(&estimation);
    physical.source = EvidenceSource::PhysicalPlant;
    let message = physical
        .check_recordings(&estimation, None)
        .expect_err("refused")
        .message;
    assert!(
        message.contains("simulated plant") && message.contains("physical plant"),
        "{message}"
    );

    // A recording whose start was not confirmed stays labelled incomplete, and no model came from it (B10.3).
    let recorded = run(
        vec![binding("processVariable", "%IL0", "LREAL")],
        vec![
            sample(1, Some(0), &[Some(20.0)], &[Quality::Good]),
            sample(2, Some(10_000_000), &[Some(20.5)], &[Quality::Good]),
        ],
    );
    let unconfirmed = recording_evidence(&recorded, Some(UNCONFIRMED_START_RECOVERED));
    let incomplete = RecordingFile::new(
        "loop:tank",
        EvidenceSource::SimulatedPlant,
        recorded,
        unconfirmed,
    );
    let from_incomplete = provisional(&incomplete);
    let message = from_incomplete
        .check_recordings(&incomplete, None)
        .expect_err("refused")
        .message;
    assert!(message.contains("not complete evidence"), "{message}");
}

#[test]
fn a_document_over_the_storage_limit_is_refused() {
    // Zero bytes are valid text, and the allocation is not touched until it is read.
    let limit = usize::try_from(STUDY_STORAGE_LIMIT_BYTES).expect("addressable");
    let oversize = String::from_utf8(vec![0_u8; limit + 1]).expect("NUL is UTF-8");
    let failure = IdentifySection::decode(&oversize).expect_err("over the limit");
    assert!(failure.message.contains("257 MiB"), "{}", failure.message);
}

/// The request of a connected record of the runtime recording `runtime_recording_id`.
fn record_request(runtime_recording_id: &str) -> StoredRecordRequest {
    let record = setpoint_record(
        runtime_recording_id,
        &[("Plant.Command", "V"), ("Plant.Position", "m")],
        &[(0.0, 1, vec![Some(1.0), Some(20.0)])],
        RequirementStatus::Pass,
    );
    StoredRecordRequest {
        project_revision: record.project_revision,
        request: record.request,
    }
}

#[test]
fn a_kept_record_request_names_the_runtime_recording_of_the_estimation_file() {
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let runtime_recording = estimation.run.recording_id.clone();
    let section = provisional(&estimation).with_record_request(record_request(&runtime_recording));
    let value = serde_json::to_value(&section).expect("json");
    assert_eq!(
        value["recordRequest"]["projectRevision"],
        json!("sha256:project")
    );
    assert_eq!(
        value["recordRequest"]["request"]["acquisition"]["recordingId"],
        json!(runtime_recording)
    );
    let text = section.encode().expect("encoded");
    assert_eq!(IdentifySection::decode(&text).expect("decoded"), section);
    section
        .check_recordings(&estimation, None)
        .expect("the request of the estimation recording");

    let mut study = Study::new("loop:tank", "sha256:loop-1");
    study.set_section(&section).expect("kept");
    assert_eq!(
        study.section::<IdentifySection>().expect("read"),
        Some(section),
        "the study keeps the request with the section"
    );

    // The request of another runtime recording is not the request of this estimation recording.
    let foreign = provisional(&estimation).with_record_request(record_request("recording-other"));
    let message = foreign
        .check_recordings(&estimation, None)
        .expect_err("refused")
        .message;
    assert!(
        message.contains("runtime recording recording-other"),
        "{message}"
    );
    // A section without a request is kept as before.
    assert_eq!(provisional(&estimation).record_request, None);
    let mut extra = serde_json::to_value(provisional(&estimation)).expect("json");
    extra["recordRequest"] = json!({ "projectRevision": "sha256:project" });
    assert!(
        IdentifySection::decode(&canonical_json(&extra)).is_err(),
        "a request without its connected request is refused"
    );
}

// Specification 35, A8.4: the provisional model is no longer current when its signals changed; the limits do not invalidate it.
#[test]
fn the_model_is_current_while_the_loop_has_the_signals_it_was_identified_with() {
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let closed_loop = provisional(&estimation);
    let loop_definition = plant_loop();
    assert!(closed_loop.is_current_for(&loop_definition));

    let mut limits = plant_loop();
    limits.limits.output_minimum = Some(-5.0);
    limits.signals.process_variable.range.maximum = 5.0;
    assert!(
        closed_loop.is_current_for(&limits),
        "changed limits keep the model current"
    );

    let mut input = plant_loop();
    input.signals.manipulated_variable.symbol = "Plant.Heater".into();
    assert!(!closed_loop.is_current_for(&input), "another plant input");

    let mut output = plant_loop();
    output.signals.process_variable.symbol = "Plant.Level".into();
    assert!(
        !closed_loop.is_current_for(&output),
        "another measured output"
    );

    let mut setpoint = plant_loop();
    if let Some(signal) = setpoint.signals.setpoint.as_mut() {
        signal.symbol = "Plant.Reference".into();
    }
    assert!(!closed_loop.is_current_for(&setpoint), "another setpoint");
    let mut without_setpoint = plant_loop();
    without_setpoint.signals.setpoint = None;
    assert!(
        !closed_loop.is_current_for(&without_setpoint),
        "no setpoint"
    );

    // An open-loop identification used no setpoint, so a changed setpoint leaves its model current.
    let mut open_loop = provisional(&estimation);
    open_loop.signals.setpoint = None;
    assert!(open_loop.is_current_for(&setpoint));
    assert!(open_loop.is_current_for(&without_setpoint));
}

// Specification 35, B3.6: only the recording of the Identify result is fitted again, from the request the section keeps.
#[test]
fn only_the_estimation_recording_whose_request_the_section_keeps_is_fitted_again() {
    let estimation = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let estimation_id = id_of(&estimation);
    let request = record_request(&estimation.run.recording_id);
    let section = provisional(&estimation).with_record_request(request.clone());
    assert_eq!(section.refit_request(&estimation_id), Ok(&request));

    let other = id_of(&recording_file(EvidenceSource::SimulatedPlant, 30.0));
    assert_eq!(
        section.refit_request(&other),
        Err(refit_refusal(&other)),
        "another recording of the study"
    );
    assert_eq!(
        provisional(&estimation).refit_request(&estimation_id),
        Err(refit_refusal(&estimation_id)),
        "a section that keeps no request"
    );
    let refusal = refit_refusal(&estimation_id);
    assert_eq!(
        refusal.message,
        format!("Recording {estimation_id} cannot be fitted again: the study keeps the experiment request only for the recording of its Identify result.")
    );
    assert_eq!(
        refusal.detail,
        Some(format!(
            "{} Identify plant records a new one.",
            refusal.message
        ))
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::ModelOrData, NextAction::Identify)
    );
}
