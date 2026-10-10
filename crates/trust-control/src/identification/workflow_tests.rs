//! Tests of saving, restoring and editing Identify's review workflow, ported from
//! `control-studio-identification-state.test.ts` and `control-studio-identification-form-draft.test.ts`, and the
//! workflow parts of `control-studio-identification-worker.test.ts`, `control-studio-saved-model-authority.test.ts`
//! and `control-studio-draft-storage.test.ts`. The parts that drive the analysis sessions, the worker, the extension
//! host or a webview are not ported. The fits themselves are tested in `workflow_fit_tests.rs`.

use serde_json::json;

use super::fixtures::{
    accepted_first_order, accepted_preview, accepted_with_originals, draft_samples, encoded,
    first_order_samples, form, grid, held_step, held_step_settings, held_step_with_recording,
    preview, recording_of, request, restored, samples, selected, settings, snapshot, test_loop,
    worker_samples, worker_settings, workflow,
};
use super::*;
use crate::experiment::ControlExperimentKind;
use crate::identification::plant::Partition;

// control-studio-identification-state.test.ts

#[test]
fn refining_a_two_lag_request_right_after_import_keeps_the_original_first_order_grid() {
    let mut imported = workflow();
    imported.select_recording(held_step_with_recording(10.0, "source-estimation"));
    assert_eq!(
        imported.current_state().phase,
        IdentificationPhase::Selected,
        "imported inputs still require Check"
    );
    let current = imported
        .snapshot()
        .and_then(|saved| saved.current)
        .expect("the import is the current input");
    assert_eq!(current.step_response_settings, Some(settings()));
    let recording = current
        .recording
        .as_ref()
        .expect("the original recording is kept");
    assert_eq!(recording.kind, ControlExperimentKind::ProjectSequence);
    assert_eq!(recording.run.recording_id, "source-estimation");
    assert!(imported.accepted_model().is_none());

    let extended = held_step_settings(grid(0.2, 1.8, 0.1), settings().time_constant);
    let input = restore_identification_input(&current);
    imported.qualify(QualifyRequest {
        samples: input.estimation,
        excluded: Vec::new(),
        source: DataSource::Simulation,
        requested_kind: IdentificationModelSelection::Sopdt,
        recording_id: input.estimation_recording_id,
        step_response_settings: Some(extended),
        recording: input.recording,
        first_order_step_response_settings: None,
    });
    let refined = imported
        .snapshot()
        .and_then(|saved| saved.current)
        .expect("the refined input is current");
    assert_eq!(
        refined.first_order_step_response_settings,
        Some(settings()),
        "refining B immediately after import must preserve the original A grid"
    );
}

#[test]
fn a_selected_validation_recording_keeps_the_estimation_and_waits_for_its_check_after_restore() {
    let mut workflow = preview();
    let estimation = snapshot(&workflow)["estimation"].clone();
    workflow.select_recording(held_step(12.0, "source-validation"));
    assert_eq!(workflow.next_fit_role(), FitRole::Validation);
    assert!(workflow.accepted_model().is_none());
    let refusal = workflow
        .fit()
        .expect_err("a selected recording is not checked yet");
    assert!(refusal.0.contains("Qualify a suitable recording"));
    // Select again after the intentionally refused fit, so that the saved state waits for its check.
    workflow.select_recording(held_step(12.0, "source-validation"));
    let saved = snapshot(&workflow);
    assert_eq!(saved["phase"], json!("selected"));
    assert_eq!(saved["estimation"], estimation);
    let mut reopened = restored(&saved);
    assert_eq!(snapshot(&reopened), saved);
    reopened.qualify(held_step(12.0, "source-validation"));
    reopened.fit().expect("the validation fits");
    let accepted = reopened.accept().expect("the validated model is accepted");
    assert_eq!(
        accepted.review.result.partition,
        Some(Partition::IndependentRuns)
    );
}

#[test]
fn a_second_fopdt_recording_keeps_the_estimation_and_its_step_settings() {
    let mut workflow = restored(&snapshot(&preview()));
    workflow.select_recording(QualifyRequest {
        first_order_step_response_settings: Some(settings()),
        ..held_step_with_recording(12.0, "second-validation")
    });
    let saved = workflow.snapshot().expect("the workflow has a saved state");
    let estimation = saved
        .estimation
        .expect("the first imported estimation must survive selecting the second recording");
    assert_eq!(
        estimation.estimation_recording_id.as_deref(),
        Some("source-estimation")
    );
    let current = saved.current.expect("the second recording is current");
    assert_eq!(
        current.estimation_recording_id.as_deref(),
        Some("second-validation")
    );
    assert_eq!(current.step_response_settings, Some(settings()));
    assert_eq!(
        current.first_order_step_response_settings, None,
        "FOPDT has no separate first-order reference fit; do not import foreign SOPDT settings"
    );
}

#[test]
fn replacing_the_recording_of_an_accepted_workflow_clears_its_authority() {
    let mut workflow = restored(&snapshot(&accepted_with_originals()));
    workflow.accept().expect("the restored review is accepted");
    assert!(workflow.accepted_model().is_some());
    let replacement = recording_of(&samples(12.0), "new-selection");
    workflow.select_recording(QualifyRequest {
        recording: Some(IdentificationRecording {
            kind: ControlExperimentKind::Observation,
            ..replacement.clone()
        }),
        ..held_step(12.0, "new-selection")
    });
    assert!(workflow.accepted_model().is_none());
    assert_eq!(workflow.next_fit_role(), FitRole::Estimation);
    let current = workflow
        .snapshot()
        .and_then(|saved| saved.current)
        .expect("the new recording is current");
    assert_eq!(
        current.estimation_recording_id.as_deref(),
        Some("new-selection")
    );
    assert_eq!(
        current.recording.map(|recording| recording.run),
        Some(replacement.run)
    );
}

#[test]
fn a_restored_preview_uses_the_next_distinct_record_for_validation() {
    let original = preview();
    let mut saved = snapshot(&original);
    assert_eq!(saved["phase"], json!("preview"));
    assert_eq!(
        saved["estimation"]["estimationRecordingId"],
        json!("source-estimation")
    );
    assert_eq!(
        saved["estimation"]["stepResponseSettings"],
        encoded(&settings())
    );
    let mut reopened = restored(&saved);
    assert_eq!(reopened.next_fit_role(), FitRole::Validation);
    assert!(reopened.accepted_model().is_none());
    reopened.qualify(held_step(12.0, "source-validation"));
    let review = reopened.fit().expect("the validation fits");
    assert!(matches!(
        review,
        IdentificationReview::AcceptanceRequired(_)
    ));
    assert!(
        reopened.accepted_model().is_none(),
        "restoring a preview must not accept a model"
    );
    reopened.accept().expect("the validated model is accepted");
    saved["estimation"]["estimation"][0]["output"] = json!(99.0);
    assert_eq!(
        snapshot(&original)["estimation"]["estimation"][0]["output"],
        json!(35.0)
    );
}

#[test]
fn qualified_validation_and_accepted_states_restore_without_fitting_a_new_estimation() {
    let mut original = preview();
    original.qualify(held_step(12.0, "source-validation"));
    let qualified = snapshot(&original);
    assert_eq!(qualified["phase"], json!("validation-qualified"));
    let mut reopened = restored(&qualified);
    let review = reopened.fit().expect("the validation fits").clone();
    reopened.accept().expect("the validated model is accepted");
    let accepted = snapshot(&reopened);
    assert_eq!(accepted["phase"], json!("accepted"));
    assert_eq!(
        accepted["current"]["estimationRecordingId"],
        json!("source-validation")
    );
    let again = restored(&accepted);
    assert_eq!(
        encoded(&again.accepted_model()),
        encoded(&Some(selected(&review)))
    );
    assert_eq!(snapshot(&again), accepted);
}

#[test]
fn a_malformed_or_invalid_saved_state_is_refused_and_leaves_the_current_analysis() {
    let mut active = preview();
    let before = snapshot(&active);
    let valid = snapshot(&accepted_preview());
    let mutations: [fn(&mut Value); 5] = [
        |value| {
            value["schemaVersion"] = json!(99);
        },
        |value| {
            value
                .as_object_mut()
                .expect("an object")
                .remove("estimation");
        },
        |value| {
            value["current"]["estimationRecordingId"] =
                value["estimation"]["estimationRecordingId"].clone();
        },
        |value| {
            let output = value["current"]["estimation"][100]["output"]
                .as_f64()
                .expect("a number");
            value["current"]["estimation"][100]["output"] = json!(output + 5.0);
        },
        |value| {
            value["estimation"]["stepResponseSettings"]["measurementBound"] = json!(0.0);
        },
    ];
    for mutate in mutations {
        let mut invalid = valid.clone();
        mutate(&mut invalid);
        assert!(active.restore(&invalid, &test_loop()).is_err());
        assert_eq!(
            snapshot(&active),
            before,
            "a failed restore cannot erase or partly replace the active preview"
        );
    }
}

#[test]
fn both_complete_original_recordings_are_kept_in_the_session() {
    let saved = snapshot(&accepted_with_originals());
    assert_eq!(
        saved["estimation"]["recording"],
        encoded(&recording_of(&samples(10.0), "source-estimation")),
        "the estimation input must retain its original complete scan record"
    );
    assert_eq!(
        saved["current"]["recording"],
        encoded(&recording_of(&samples(12.0), "source-validation"))
    );
    assert_eq!(snapshot(&restored(&saved)), saved);
}

#[test]
fn a_saved_study_binds_its_inputs_to_the_original_acquisition() {
    let saved = snapshot(&accepted_with_originals());
    assert!(decode_identification_state(&saved, &test_loop()).is_some());
    let mutations: [fn(&mut Value); 3] = [
        |value| {
            value["schemaVersion"] = json!(99);
        },
        |value| {
            let values = &mut value["estimation"]["recording"]["run"]["samples"][20]["values"];
            let measured = values[0].as_f64().expect("a number");
            values[0] = json!(measured + 10.0);
        },
        |value| {
            value["estimation"]["recording"]["run"]["recording_id"] = json!("other");
        },
    ];
    for mutate in mutations {
        let mut invalid = saved.clone();
        mutate(&mut invalid);
        assert_eq!(
            decode_identification_state(&invalid, &test_loop()),
            None,
            "saved analysis must bind its inputs to the original acquisition"
        );
    }
}

#[test]
fn new_identification_inputs_clear_the_accepted_model() {
    let mut workflow = restored(&snapshot(&accepted_with_originals()));
    workflow.accept().expect("the restored review is accepted");
    assert!(workflow.accepted_model().is_some());
    workflow.qualify(held_step_with_recording(12.0, "next"));
    assert!(
        workflow.accepted_model().is_none(),
        "an old accepted model cannot remain active after inputs change"
    );
}

// control-studio-identification-form-draft.test.ts

#[test]
fn an_initial_form_only_editing_state_claims_no_acquisition() {
    let mut workflow = workflow();
    workflow.edit_form(form());
    let saved = snapshot(&workflow);
    assert_eq!(saved["phase"], json!("editing"));
    assert_eq!(saved["form"], encoded(&form()));
    assert!(saved.get("current").is_none());
    assert!(saved.get("estimation").is_none());
    assert_eq!(snapshot(&restored(&saved)), saved);
}

#[test]
fn editing_an_accepted_workflow_clears_authority_but_keeps_the_original_records() {
    let mut workflow = accepted_first_order(120);
    let before = snapshot(&workflow);
    workflow.edit_form(form());
    let after = snapshot(&workflow);
    assert!(workflow.accepted_model().is_none());
    assert_eq!(after["phase"], json!("editing"));
    assert_eq!(after["form"], encoded(&form()));
    assert_eq!(
        after["current"]["estimationRecordingId"],
        before["current"]["estimationRecordingId"]
    );
    assert!(after.get("estimation").is_none());
    assert_eq!(snapshot(&restored(&after)), after);
}

#[test]
fn selecting_another_recording_keeps_the_pending_form() {
    let mut workflow = workflow();
    workflow.edit_form(form());
    workflow.select_recording(request(
        first_order_samples(120, 10.0),
        IdentificationModelSelection::Fopdt,
        Some("recording-1"),
        None,
    ));
    let saved = snapshot(&workflow);
    assert_eq!(saved["phase"], json!("selected"));
    assert_eq!(saved["form"], encoded(&form()));
    assert_eq!(
        saved["current"]["estimationRecordingId"],
        json!("recording-1")
    );
    assert_eq!(snapshot(&restored(&saved)), saved);
}

#[test]
fn editing_a_preview_keeps_its_estimation_record_and_restores_it() {
    let mut workflow = workflow();
    workflow.qualify(request(
        first_order_samples(120, 10.0),
        IdentificationModelSelection::FirstOrder,
        Some("preview-estimation"),
        None,
    ));
    workflow.fit().expect("the estimation fits");
    workflow.edit_form(form());
    let saved = snapshot(&workflow);
    assert_eq!(saved["phase"], json!("editing"));
    assert_eq!(
        saved["estimation"]["estimationRecordingId"],
        json!("preview-estimation")
    );
    assert_eq!(snapshot(&restored(&saved)), saved);
}

#[test]
fn editing_a_rejected_current_record_keeps_that_record_through_restore() {
    let mut workflow = workflow();
    workflow.qualify(request(
        first_order_samples(120, 0.0),
        IdentificationModelSelection::FirstOrder,
        Some("rejected-record"),
        None,
    ));
    workflow.edit_form(form());
    let saved = snapshot(&workflow);
    assert_eq!(saved["phase"], json!("editing"));
    assert_eq!(
        saved["current"]["estimationRecordingId"],
        json!("rejected-record")
    );
    assert_eq!(snapshot(&restored(&saved)), saved);
}

// control-studio-identification-worker.test.ts

#[test]
fn a_replayed_fit_reproduces_the_saved_review() {
    // The `TypeScript` test replays a two-lag sensor-lag comparison, which is parked with stage V7; the same replay is
    // checked here on the bounded first-order fit of the same recording.
    let mut source = workflow();
    source.qualify(request(
        worker_samples(10.0),
        IdentificationModelSelection::Fopdt,
        Some("saved-estimation"),
        Some(worker_settings()),
    ));
    source.fit().expect("the estimation fits");
    let saved = snapshot(&source);
    let reopened = restored(&saved);
    assert_eq!(snapshot(&reopened), saved);
    assert_eq!(
        encoded(&reopened.current_state().review),
        encoded(&source.current_state().review)
    );
}

#[test]
fn a_malformed_saved_state_leaves_the_running_analysis_alive() {
    let mut workflow = workflow();
    workflow.qualify(held_step(10.0, "estimation"));
    let before = snapshot(&workflow);
    let refusal = workflow
        .restore(&json!({ "schemaVersion": 99 }), &test_loop())
        .expect_err("a malformed study is refused");
    assert_eq!(
        refusal.0,
        "Saved identification inputs are malformed or incompatible."
    );
    assert_eq!(snapshot(&workflow), before);
    workflow.fit().expect("the estimation still fits");
    assert_eq!(workflow.current_state().phase, IdentificationPhase::Preview);
}

// control-studio-saved-model-authority.test.ts, control-studio-draft-storage.test.ts

#[test]
fn an_accepted_first_order_study_restores_with_its_model() {
    let original = accepted_first_order(100);
    let saved = snapshot(&original);
    let reopened = restored(&saved);
    assert_eq!(snapshot(&reopened), saved);
    assert_eq!(
        encoded(&reopened.accepted_model()),
        encoded(&original.accepted_model())
    );
}

#[test]
fn a_qualified_study_with_its_retained_recording_decodes_and_restores() {
    let rows = draft_samples();
    let mut workflow = workflow();
    workflow.qualify(request(
        rows.clone(),
        IdentificationModelSelection::FirstOrder,
        Some("recording-1"),
        None,
    ));
    let mut saved = snapshot(&workflow);
    assert_eq!(saved["phase"], json!("qualified"));
    saved["current"]["recording"] = encoded(&recording_of(&rows, "recording-1"));
    let decoded = decode_identification_state(&saved, &test_loop())
        .expect("a study with its retained recording decodes");
    let recording = decoded
        .current
        .and_then(|current| current.recording)
        .expect("the recording is kept");
    assert_eq!(recording.run.recording_id, "recording-1");
    assert_eq!(snapshot(&restored(&saved)), saved);
}
