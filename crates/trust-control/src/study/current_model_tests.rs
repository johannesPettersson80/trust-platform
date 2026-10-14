//! Tests of the study's current model (specification 35, B3.7 and A8.4): which of a study's two models is current, what a supplied model
//! is, the stage evidence and the Tune that read it, and the identification `controlStudio/open` answers for it.

use serde_json::json;

use super::*;
use crate::identification::identify_result::study_identification;
use crate::loop_definition::LoopDefinition;
use crate::messages::TuneRequest;
use crate::model::decode_general_model;
use crate::stages::{stage_availability, Stage, StageEvidence, StageStatus};
use crate::study::general_model_persistence::SuppliedModelSection;
use crate::study::inputs::stage_evidence_of;
use crate::study::tune::model::tests::identify_section;
use crate::study::tune::{check_before_start, tune, TuneInputs};

/// The recording the fixture fit names, as `tune/model_tests.rs` gives it.
const ESTIMATION_RECORDING: &str = "rec-00112233445566778899aabbccddeeff";

/// The commissioning example's loop, whose signals the fixtures use.
fn commissioning_loop() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop")
}

/// A supplied model `1.5·e^(−2s)/(4s + 1)` from `P1.ManipulatedValue` in % to `P1.ProcessValue` in °C at 50 % and 50 °C, the model of
/// the fixture fit. Its validation is `validation`: "validated" with evidence, or "unvalidated" (provisional).
fn supplied_model(validation: &str) -> SuppliedModelSection {
    let evidence = if validation == "validated" {
        json!(["independent-course-measurement"])
    } else {
        json!([])
    };
    let model = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "provided-course-model", "source": "provided",
        "validation": validation, "evidence": evidence, "delaySeconds": 2.0,
        "inputs": [{ "id": "P1.ManipulatedValue", "unit": "%", "operatingRange": { "minimum": 0.0, "maximum": 100.0 } }],
        "outputs": [{ "id": "P1.ProcessValue", "unit": "°C", "operatingRange": { "minimum": 0.0, "maximum": 100.0 } }],
        "operatingPoint": { "inputs": [50.0], "outputs": [50.0] },
        "numerator": [1.5], "denominator": [4.0, 1.0]
    }))
    .expect("a supplied model");
    SuppliedModelSection::new(model, None)
}

/// A study of the commissioning loop that holds `supplied`, as `importModel` keeps it.
fn study_with_supplied(loop_definition: &LoopDefinition, supplied: &SuppliedModelSection) -> Study {
    let mut study = Study::new(&loop_definition.id, &loop_definition.revision);
    study
        .set_section(supplied)
        .expect("the supplied model is kept");
    study.model_source = Some(CurrentModelSource::Supplied);
    study
}

/// The Tune request of these tests: Automatic, with no response, pins or settings.
fn automatic_request() -> TuneRequest {
    TuneRequest {
        project_root: "/projects/control-pid-commissioning".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "control-pid-commissioning".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method: None,
        choice: None,
        structure: None,
        pins: None,
        settings: None,
        connection: None,
    }
}

#[test]
fn a_supplied_model_was_never_fitted_so_it_has_no_fit_error_and_is_current_for_its_study() {
    let current = CurrentModel::supplied(supplied_model("unvalidated"));
    assert_eq!(current.source, CurrentModelSource::Supplied);
    assert!(current.identify.is_none());
    assert!(
        current.provisional(),
        "a model without independent validation is provisional (B3.7)"
    );
    assert_eq!(
        current.fit_error(),
        None,
        "a supplied model has no fit error (B3.7)"
    );
    assert!(current.is_current_for(&commissioning_loop()));
    assert!(
        !CurrentModel::supplied(supplied_model("validated")).provisional(),
        "a model with independent validation is not provisional (B3.7)"
    );
}

#[test]
fn a_fit_is_decided_by_the_error_its_presentation_records() {
    let provisional = CurrentModel::identified(identify_section(None));
    assert!(provisional.provisional());
    assert_eq!(
        provisional.fit_error(),
        Some(0.04),
        "a provisional fit is decided by its estimation error (N3.2)"
    );
    let validated = CurrentModel::identified(identify_section(Some(0.12)));
    assert!(!validated.provisional());
    assert_eq!(
        validated.fit_error(),
        Some(0.12),
        "a validated fit is decided by its validation error (N3.2)"
    );
}

#[test]
fn the_marker_decides_which_model_a_study_holding_both_makes_current() {
    let loop_definition = commissioning_loop();
    let mut study = Study::new(&loop_definition.id, &loop_definition.revision);
    assert_eq!(
        current_model(&study).expect("reads"),
        None,
        "a study with neither model has no current model, as after Reset"
    );
    study.set_section(&identify_section(None)).expect("kept");
    study
        .set_section(&supplied_model("unvalidated"))
        .expect("kept");
    let source = |study: &Study| {
        current_model(study)
            .expect("reads")
            .expect("a current model")
            .source
    };
    assert_eq!(
        source(&study),
        CurrentModelSource::Identify,
        "without a marker the fit is current"
    );
    study.model_source = Some(CurrentModelSource::Supplied);
    assert_eq!(source(&study), CurrentModelSource::Supplied);
    study.model_source = Some(CurrentModelSource::Identify);
    assert_eq!(source(&study), CurrentModelSource::Identify);
}

#[test]
fn after_an_import_the_study_has_a_model_and_tune_is_available_on_it() {
    let loop_definition = commissioning_loop();
    let study = study_with_supplied(&loop_definition, &supplied_model("unvalidated"));
    let evidence = stage_evidence_of(&loop_definition, &study).expect("the evidence reads");
    assert!(
        evidence.has_model,
        "a supplied model is the study's model (B3.7)"
    );
    let tune_stage = stage_availability(&evidence)
        .into_iter()
        .find(|stage| stage.stage == Stage::Tune)
        .expect("the Tune stage");
    assert!(
        matches!(tune_stage.status, StageStatus::Ready),
        "Tune is ready on the model before any candidate (B4.2)"
    );
}

#[test]
fn tune_designs_on_a_supplied_model_whose_error_was_never_recorded() {
    let loop_definition = commissioning_loop();
    let current = CurrentModel::supplied(supplied_model("validated"));
    let request = automatic_request();
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&current),
        previous: None,
        request: &request,
        relay: None,
    };
    check_before_start(&inputs)
        .expect("a supplied model is tuned: Tune's error check does not apply (B3.7)");
    let outcome = tune(&inputs).expect("Automatic designs a candidate on the supplied model");
    assert_eq!(outcome.section.model_revision, "provided-course-model");
    assert!(
        !outcome.section.candidate.applied,
        "Tune never writes (B4.1)"
    );
}

#[test]
fn the_reopened_identification_is_the_supplied_model_until_a_later_fit_replaces_it() {
    let loop_definition = commissioning_loop();
    let mut study = study_with_supplied(&loop_definition, &supplied_model("unvalidated"));
    let answer = study_identification(&study, &loop_definition, StageEvidence::default())
        .expect("the study reads")
        .expect("the supplied model is the identification the study holds");
    assert_eq!(answer.model_revision, "provided-course-model");
    assert!(
        answer.provisional,
        "a supplied model without validation is provisional"
    );
    assert_eq!(
        answer.recording_id, None,
        "a supplied model has no recording"
    );
    assert!(
        answer.equation.starts_with("G(s) = "),
        "{}",
        answer.equation
    );

    study
        .set_section(&identify_section(None))
        .expect("the fit is kept");
    study.model_source = Some(CurrentModelSource::Identify);
    let fit = study_identification(&study, &loop_definition, StageEvidence::default())
        .expect("the study reads")
        .expect("the fit is the identification the study holds");
    assert_eq!(
        fit.model_revision, "sha256:fopdt-model",
        "a later Identify makes its fit the current model (A8.4)"
    );
    assert_eq!(fit.recording_id.as_deref(), Some(ESTIMATION_RECORDING));
}
