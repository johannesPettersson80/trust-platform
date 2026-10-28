#![allow(clippy::float_cmp)] // The values are the decimals that the record holds and the mapping keeps, compared as written.

use std::collections::BTreeMap;
use std::fs;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::analysis::parameter_writes::{writable_parameters, WritableParameter};
use trust_control::commissioning::parameter_status::{ParameterStatus, RecordState};
use trust_control::experiment::{
    ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
};
use trust_control::identification::connected_run::loop_busy_refusal;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{OperationState, OperationUpdate, ParameterWorkRequest};

use super::*;
use crate::journal::JournalEntry;
use crate::operation::UpdateSink;
use crate::runtime_client::fake_runtime::{answer as reply, refusal, tank_loop, FakeRuntime};

/// Lesson 45 (root locus): Kp is swept and selected at 16, Kd is fixed at 7, and both are mapped to program symbols.
const LESSON_45: &str = include_str!(
    "../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/control-loop.json"
);

fn shaft() -> LoopDefinition {
    LoopDefinition::decode(LESSON_45).expect("the shipped lesson decodes")
}

fn binding(loop_definition: &LoopDefinition) -> RuntimeBinding {
    binding_of(loop_definition).expect("lesson 45 declares its binding")
}

fn writable(loop_definition: &LoopDefinition) -> Vec<WritableParameter> {
    writable_parameters(loop_definition).expect("the mapping of lesson 45 is writable")
}

/// The record of the runtime: the two mapped parameters and one unmapped parameter of the binding.
fn record(kp: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.Kp".to_string(), kp),
        ("P1.Kd".to_string(), kd),
        ("P1.Other".to_string(), 5.0),
    ])
}

/// The binding of lesson 45 has no unmapped parameter, so the record of the test holds the same symbols.
fn record_of_binding(kp: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([("P1.Kp".to_string(), kp), ("P1.Kd".to_string(), kd)])
}

fn status(values: BTreeMap<String, f64>, state: RecordState) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: None,
        program_revision: "program-1".to_string(),
        binding_revision: "binding-1".to_string(),
        active_revision: "active-2".to_string(),
        values,
        status: state,
        previous_revision: None,
        previous_values: None,
        reason: None,
    }
}

fn proposed(kp: f64, kd: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([("P1.Kp".to_string(), kp), ("P1.Kd".to_string(), kd)])
}

#[test]
fn the_review_of_an_apply_names_the_whole_record_it_would_set() {
    let loop_definition = shaft();
    let binding = binding(&loop_definition);
    let writable = writable(&loop_definition);
    let observed = status(record_of_binding(1.0, 7.0), RecordState::Idle);
    let planned = plan_apply(
        &loop_definition,
        &writable,
        &binding,
        observed,
        &proposed(4.0, 7.0),
        "transaction-test".to_string(),
    )
    .expect("4 is inside the analysis range of Kp");
    assert_eq!(planned.review.values["P1.Kp"], 4.0);
    assert_eq!(planned.review.values["P1.Kd"], 7.0);
    assert_eq!(planned.review.transaction_id, "transaction-test");
    assert!(planned.identity.starts_with("sha256:"));
    let kp = planned
        .changes
        .iter()
        .find(|change| change.symbol == "P1.Kp")
        .expect("Kp changes");
    assert_eq!((kp.running, kp.next), (1.0, 4.0));
}

#[test]
fn a_value_outside_the_analysis_range_is_refused_before_the_review_is_made() {
    let loop_definition = shaft();
    let error = plan_apply(
        &loop_definition,
        &writable(&loop_definition),
        &binding(&loop_definition),
        status(record_of_binding(1.0, 7.0), RecordState::Idle),
        &proposed(21.0, 7.0),
        String::new(),
    )
    .err()
    .expect("21 is above the analysis maximum");
    assert!(
        error.message.contains("is outside the range"),
        "{}",
        error.message
    );
}

#[test]
fn a_pending_record_refuses_the_review() {
    let loop_definition = shaft();
    let error = plan_apply(
        &loop_definition,
        &writable(&loop_definition),
        &binding(&loop_definition),
        status(record_of_binding(1.0, 7.0), RecordState::Pending),
        &proposed(4.0, 7.0),
        String::new(),
    )
    .err()
    .expect("a pending record is not reviewed");
    assert!(error.message.contains("pending"), "{}", error.message);
}

/// A write of lesson 45's loop that confirms the review `identity` with `values`.
fn write_of(identity: &str, values: BTreeMap<String, f64>) -> Write {
    Write {
        loop_definition: shaft(),
        identity: identity.to_owned(),
        values,
        storage_folder: PathBuf::new(),
        connection: RuntimeConnection {
            endpoint: "tcp://127.0.0.1:9".to_owned(),
            token: None,
            runtime_id: None,
        },
        physical_approved: false,
    }
}

#[test]
fn a_review_identity_that_no_longer_matches_refuses_the_write() {
    let loop_definition = shaft();
    let record = status(record_of_binding(1.0, 7.0), RecordState::Idle);
    let reviewed = plan_apply(
        &loop_definition,
        &writable(&loop_definition),
        &binding(&loop_definition),
        record.clone(),
        &proposed(4.0, 7.0),
        String::new(),
    )
    .expect("the review");
    let named = |identity: &str| {
        named_write(
            &write_of(identity, proposed(4.0, 7.0)),
            &writable(&loop_definition),
            &binding(&loop_definition),
            record.clone(),
            "transaction-1",
        )
    };
    assert!(
        named(&reviewed.identity).is_ok(),
        "the same record gives the same identity"
    );
    let error = named("sha256:when-reviewed")
        .err()
        .expect("the record changed since the review");
    assert_eq!(
        error.detail.as_deref().unwrap_or(error.message.as_str()),
        REVIEW_CHANGED
    );
    assert_eq!(error.cause, Cause::Approval);
    assert_eq!(error.next_action, NextAction::None);
}

#[test]
fn a_restore_reviews_the_values_retained_before_the_last_write() {
    let loop_definition = shaft();
    let writable = writable(&loop_definition);
    let mut retained = status(record_of_binding(4.0, 7.0), RecordState::Applied);
    retained.previous_revision = Some("active-1".to_string());
    retained.previous_values = Some(record_of_binding(1.0, 7.0));
    let planned = plan_restore(
        &loop_definition,
        &writable,
        retained,
        "transaction-restore".to_string(),
    )
    .expect("a confirmed previous record is retained");
    assert_eq!(planned.review.values, record_of_binding(1.0, 7.0));
    assert!(planned.review.is_rollback());
    let kp = planned
        .changes
        .iter()
        .find(|change| change.symbol == "P1.Kp")
        .expect("Kp changes");
    assert_eq!((kp.running, kp.next), (4.0, 1.0));
}

#[test]
fn a_restore_is_refused_when_no_previous_record_is_retained() {
    let loop_definition = shaft();
    let error = plan_restore(
        &loop_definition,
        &writable(&loop_definition),
        status(record_of_binding(4.0, 7.0), RecordState::Applied),
        String::new(),
    )
    .err()
    .expect("nothing is retained to restore");
    assert_eq!(error.cause, Cause::Runtime);
    assert_eq!(error.next_action, NextAction::None);
}

#[test]
fn an_apply_and_a_restore_of_the_same_record_have_different_identities() {
    let loop_definition = shaft();
    let writable = writable(&loop_definition);
    let binding = binding(&loop_definition);
    let mut retained = status(record_of_binding(4.0, 7.0), RecordState::Applied);
    retained.previous_revision = Some("active-1".to_string());
    retained.previous_values = Some(record_of_binding(1.0, 7.0));
    let restore = plan_restore(&loop_definition, &writable, retained.clone(), String::new())
        .expect("restore");
    let apply = plan_apply(
        &loop_definition,
        &writable,
        &binding,
        retained,
        &proposed(1.0, 7.0),
        String::new(),
    )
    .expect("apply");
    assert_ne!(restore.identity, apply.identity);
}

#[test]
fn the_mapped_values_are_the_writable_symbols_of_the_record() {
    let loop_definition = shaft();
    let values = mapped_values(&writable(&loop_definition), &record(4.0, 7.0));
    assert_eq!(
        values,
        BTreeMap::from([("P1.Kd".to_string(), 7.0), ("P1.Kp".to_string(), 4.0)]),
        "the unmapped P1.Other is not offered for a review"
    );
}

#[test]
fn the_review_answer_names_the_identity_the_changes_the_values_and_the_target_only() {
    let loop_definition = shaft();
    let planned = plan_apply(
        &loop_definition,
        &writable(&loop_definition),
        &binding(&loop_definition),
        status(record_of_binding(1.0, 7.0), RecordState::Idle),
        &proposed(4.0, 7.0),
        String::new(),
    )
    .expect("the review");
    let answer = review_answer(
        planned,
        ApplySource::Simulation,
        Some(proposed(4.0, 7.0)),
        APPLY_REVIEWED,
    )
    .expect("the answer");
    assert_eq!(answer["physical"], json!(false));
    assert_eq!(
        answer["values"],
        json!({ "P1.Kd": 7.0, "P1.Kp": 4.0 }),
        "the review answers the values it read, which the start sends back"
    );
    assert!(answer["identity"]
        .as_str()
        .is_some_and(|identity| identity.starts_with("sha256:")));
    assert_eq!(answer["changes"].as_array().map(Vec::len), Some(2));
    assert!(
        answer.get("observed").is_none(),
        "a review writes and reads no record for the page"
    );
}

/// A `review` request for lesson 45's loop, with the fields of `extra` besides.
fn review_request(extra: &Value) -> ParameterWorkRequest {
    let mut request = json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "control-root-locus",
        "storageFolder": "",
        "action": "review"
    });
    request
        .as_object_mut()
        .expect("a request is an object")
        .extend(extra.as_object().expect("the fields are an object").clone());
    serde_json::from_value(request).expect("the request decodes")
}

/// C12: the page sends the values as the engineer typed them, and the engine reads the text and refuses one that is none.
#[test]
fn the_values_of_a_review_are_read_from_the_text_the_engineer_typed() {
    let typed = review_request(&json!({ "json": r#"{"P1.Kp": 4, "P1.Kd": 7}"# }));
    assert_eq!(
        proposed_values(&typed).expect("an object of numbers"),
        proposed(4.0, 7.0)
    );
    let values = review_request(&json!({ "values": { "P1.Kp": 2, "P1.Kd": 7 } }));
    assert_eq!(
        proposed_values(&values).expect("the request's values"),
        proposed(2.0, 7.0),
        "a request without a text reviews its values"
    );
    let both =
        review_request(&json!({ "json": r#"{"P1.Kp": 4, "P1.Kd": 7}"#, "values": { "P1.Kp": 2 } }));
    assert_eq!(
        proposed_values(&both).expect("the text"),
        proposed(4.0, 7.0),
        "the text the engineer typed is the review's values"
    );
}

#[test]
fn a_review_text_that_is_no_object_of_numbers_is_refused_with_the_engines_sentence() {
    let not_json = review_request(&json!({ "json": "{" }));
    let refusal = proposed_values(&not_json).expect_err("a text that is not JSON");
    assert_eq!(refusal.message, "The parameter values are not valid JSON.");
    assert_eq!(refusal.cause, Cause::Settings);
    assert_eq!(refusal.next_action, NextAction::None);
    let list = review_request(&json!({ "json": "[4, 7]" }));
    assert_eq!(
        proposed_values(&list)
            .expect_err("JSON that is no object")
            .message,
        "Provide exactly the declared finite parameter values."
    );
}

#[test]
fn the_request_names_its_actions_in_camel_case() {
    let request: ParameterWorkRequest = serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "control-root-locus",
        "storageFolder": "",
        "action": "reviewRestore",
        "physicalApproved": false
    }))
    .expect("the request decodes");
    assert_eq!(request.action, ParameterWorkAction::ReviewRestore);
    assert!(request.values.is_empty());
    assert!(request.json.is_none());
    assert!(request.review.is_none());
    let unknown: Result<ParameterWorkRequest, _> = serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "control-root-locus",
        "storageFolder": "",
        "action": "reviewRestore",
        "unknown": true
    }));
    assert!(unknown.is_err(), "an unknown field is refused");
    let value: Value = serde_json::to_value(ParameterWorkAction::Confirm).expect("the action");
    assert_eq!(value, json!("confirm"));
}

/// C12: an edit after a review spends the review, and the engine answers the sentence the page shows, with the review as the tool's next
/// step. Nothing is written, and no runtime is read.
#[test]
fn an_edit_after_a_review_is_answered_with_the_sentence_the_page_shows() {
    let loop_definition = experiment();
    let folder = scratch("invalidate");
    let request = released_request(&loop_definition, &folder);
    let answer = invalidate(&LoopClaims::default(), &loop_definition, &request)
        .expect("the edit is answered");
    assert_eq!(
        answer,
        json!({
            "message": "Values changed. Review again before applying.",
            "offer": { "nextStep": "review", "available": ["inspect", "review", "restore"] }
        })
    );
    assert_eq!(
        serde_json::to_value(ParameterWorkAction::Invalidate).expect("the action"),
        json!("invalidate")
    );
}

/// C12: a draft answers the values the analysis selects, with the note that the runtime has not changed.
#[test]
fn a_draft_answers_its_values_with_the_copy_note() {
    let answer = draft(&shaft()).expect("the draft");
    assert_eq!(
        answer["message"],
        "Selected analysis values copied for review. The runtime has not changed.",
        "{answer}"
    );
    assert!(answer["values"].is_object(), "{answer}");
}

#[test]
fn the_engine_answers_the_parameters_method() {
    assert!(
        crate::commands::METHODS.contains(&crate::commands::PARAMETERS),
        "the engine answers {}",
        crate::commands::PARAMETERS
    );
}

// The experiment's settings (specification 35, C12, Parameter purpose and Experiment settings). Lesson 21 declares them and maps no
// analysis, so its tool is the loop's own binding.

/// Lesson 21 (safe informative experiment): a project-scripted open-loop experiment that declares `parameterPurpose: "experiment"`. Its
/// settings are the step size (5 to 15 percentage points) and the hold (20 to 40 s), both `REAL`.
const LESSON_21: &str = include_str!(
    "../../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment/control-loop.json"
);
const STEP: &str = "P1.SelectedStepPoints";
const HOLD: &str = "P1.HoldSeconds";

fn experiment() -> LoopDefinition {
    LoopDefinition::decode(LESSON_21).expect("the shipped lesson decodes")
}

/// The two settings with the values given.
fn settings_of(step: f64, hold: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([(STEP.to_string(), step), (HOLD.to_string(), hold)])
}

/// The folder of one test's journals and project: empty, and not created yet.
fn scratch(name: &str) -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("trust-parameters-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    folder
}

#[test]
fn the_experiment_settings_are_the_loops_whole_binding_with_each_parameters_own_bounds() {
    let loop_definition = experiment();
    assert_eq!(
        parameter_tool(&loop_definition),
        Some(ParameterTool::Experiment)
    );
    let settings = tool_parameters(&loop_definition).expect("the settings are the loop's binding");
    assert_eq!(
        settings
            .iter()
            .map(|setting| (
                setting.symbol.as_str(),
                setting.minimum,
                setting.maximum,
                setting.unit.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            (STEP, 5.0, 15.0, "percentage-point"),
            (HOLD, 20.0, 40.0, "s")
        ]
    );
    assert_eq!(
        writable_binding(&loop_definition, &settings),
        loop_definition.parameter_binding,
        "the page shows the loop's own binding, with the loop's bounds"
    );
    let analysis = shaft();
    assert_eq!(parameter_tool(&analysis), Some(ParameterTool::Analysis));
    assert_eq!(
        tool_parameters(&analysis)
            .expect("the mapping of lesson 45 is writable")
            .len(),
        2,
        "a loop that maps an analysis keeps the analysis's parameters"
    );
}

#[test]
fn a_review_of_the_experiment_settings_names_the_whole_record_and_rounds_a_real_to_single_precision(
) {
    let loop_definition = experiment();
    let settings = tool_parameters(&loop_definition).expect("the settings");
    let binding = binding_of(&loop_definition).expect("lesson 21 declares its binding");
    let planned = plan_apply(
        &loop_definition,
        &settings,
        &binding,
        status(settings_of(10.0, 30.0), RecordState::Idle),
        &settings_of(10.1, 35.0),
        "transaction-test".to_string(),
    )
    .expect("both values are inside their bounds");
    assert_eq!(
        planned.review.values[STEP],
        f64::from(10.1_f32),
        "a REAL is rounded to single precision (C0)"
    );
    assert_eq!(planned.review.values[HOLD], 35.0);
    assert!(planned.identity.starts_with("sha256:"));
    assert_eq!(
        planned.changes.len(),
        2,
        "every setting is part of the record"
    );
    let hold = planned
        .changes
        .iter()
        .find(|change| change.symbol == HOLD)
        .expect("the hold changes");
    assert_eq!((hold.running, hold.next), (30.0, 35.0));
}

#[test]
fn the_experiment_settings_refuse_a_value_outside_their_bounds_and_a_record_that_is_not_exactly_theirs(
) {
    let loop_definition = experiment();
    let settings = tool_parameters(&loop_definition).expect("the settings");
    let binding = binding_of(&loop_definition).expect("the binding");
    let refused = |proposed: &BTreeMap<String, f64>| {
        plan_apply(
            &loop_definition,
            &settings,
            &binding,
            status(settings_of(10.0, 30.0), RecordState::Idle),
            proposed,
            String::new(),
        )
        .err()
        .expect("the review is refused")
    };
    let high = refused(&settings_of(12.0, 41.0));
    assert_eq!(
        high.message,
        "P1.HoldSeconds is outside its declared bounds after IEC rounding."
    );
    assert_eq!(high.cause, Cause::Settings);
    assert!(
        high.detail
            .as_deref()
            .is_some_and(|detail| !detail.contains("analysis")),
        "the words are the binding's, not an analysis's: {high:?}"
    );
    assert_eq!(
        refused(&settings_of(4.0, 30.0)).message,
        "P1.SelectedStepPoints is outside its declared bounds after IEC rounding."
    );
    let missing = BTreeMap::from([(STEP.to_string(), 12.0)]);
    assert_eq!(
        refused(&missing).message,
        "Provide exactly the declared finite parameter values."
    );
    let mut extra = settings_of(12.0, 35.0);
    extra.insert("P1.Other".to_string(), 1.0);
    assert_eq!(
        refused(&extra).message,
        "Provide exactly the declared finite parameter values."
    );
}

#[test]
fn the_experiment_settings_have_no_draft_because_no_analysis_selects_their_values() {
    let refusal = draft(&experiment()).expect_err("no analysis selects a value");
    assert_eq!(
        refusal
            .detail
            .as_deref()
            .unwrap_or(refusal.message.as_str()),
        NO_DRAFT
    );
    assert_eq!(refusal.cause, Cause::Settings);
    assert_eq!(refusal.next_action, NextAction::None);
}

/// The owner of a recording of the loop, as a project test journals it before the recording's start is dispatched (E11.2).
fn recording_owner(loop_definition: &LoopDefinition, recording_id: &str) -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: recording_id.to_string(),
        project_id: loop_definition.project_id.clone(),
        runtime_id: loop_definition.runtime_id.clone(),
        loop_id: loop_definition.id.clone(),
        loop_revision: loop_definition.revision.clone(),
        source: ControlEvidenceSource::Simulation,
        program_revision: None,
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "processVariable".to_string(),
            address: Some("%IL0".to_string()),
            symbol: None,
            iec_type: "LREAL".to_string(),
        }],
    }
}

// C12: the project test holds the settings, from its start to its terminal restoration. While it runs, an operation of the engine works
// on the loop and the loop's claim refuses a review or a write, naming the test.
#[test]
fn a_project_test_that_runs_freezes_the_settings_until_it_ends() {
    let loop_definition = experiment();
    let folder = scratch("frozen-running");
    let storage = folder.to_str().expect("a UTF-8 folder");
    let claims = LoopClaims::default();
    let test = claims
        .claim(
            &loop_definition.project_id,
            &loop_definition.id,
            "Identify plant",
        )
        .expect("a free loop");
    for operation in [REVIEW_OPERATION, WRITE_OPERATION] {
        let Err(refusal) = hold_settings(&claims, &loop_definition, storage, operation) else {
            panic!("{operation} is refused while the test runs");
        };
        assert_eq!(refusal, loop_busy_refusal("Identify plant"));
    }
    drop(test);
    assert!(
        hold_settings(&claims, &loop_definition, storage, REVIEW_OPERATION).is_ok(),
        "the settings are free once the test has ended"
    );
}

// C12, A8.5, A7: a test that ended without its restoration confirmed leaves its experiment journaled, so the settings stay frozen
// until the restoration is confirmed and the journal is cleared.
#[test]
fn a_project_test_whose_restoration_is_not_confirmed_keeps_the_settings_frozen_until_its_journal_is_cleared(
) {
    let loop_definition = experiment();
    let folder = scratch("frozen-journaled");
    let storage = folder.to_str().expect("a UTF-8 folder");
    let claims = LoopClaims::default();
    let mut journal = Journal::open(&folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the loop's journal");
    journal
        .record(JournalEntry::RecordingOwner {
            owner: recording_owner(&loop_definition, "recording-9"),
        })
        .expect("the test's recording is journaled");
    let Err(refusal) = hold_settings(&claims, &loop_definition, storage, REVIEW_OPERATION) else {
        panic!("an unfinished operation freezes the settings");
    };
    assert_eq!(refusal.message, "This loop has an unfinished operation.");
    assert_eq!(refusal.next_action, NextAction::Resume);
    assert!(
        refusal
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Journaled: recording-9.")),
        "{refusal:?}"
    );
    assert!(
        claims
            .claim(
                &loop_definition.project_id,
                &loop_definition.id,
                "Another operation"
            )
            .is_ok(),
        "a refused request holds nothing"
    );
    journal
        .clear("recording-9")
        .expect("the restoration is confirmed and the journal cleared");
    assert!(
        hold_settings(&claims, &loop_definition, storage, REVIEW_OPERATION).is_ok(),
        "the settings are free once the journal is clear"
    );
    let _ = fs::remove_dir_all(&folder);
}

/// A `released` request for `loop_definition`, with the folder of its journals.
fn released_request(loop_definition: &LoopDefinition, folder: &Path) -> ParameterWorkRequest {
    serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": loop_definition.runtime_id,
        "loopId": loop_definition.id,
        "storageFolder": folder.display().to_string(),
        "action": "released"
    }))
    .expect("the request decodes")
}

// C12: the answer to `released` is the sentence once the test's restoration is confirmed, whether the test finished, was aborted or
// failed, and the reason while the settings are still held, each with the tool's offer that follows: a review once the settings are
// released, another inspection while they are held. It reads no runtime.
#[test]
fn the_settings_are_released_once_nothing_works_on_the_loop_and_its_journal_is_clear() {
    let loop_definition = experiment();
    let folder = scratch("released");
    let claims = LoopClaims::default();
    let request = released_request(&loop_definition, &folder);
    let released_now = || released(&claims, &loop_definition, &request).expect("answered");
    let free = json!({
        "settingsReleased": SETTINGS_RELEASED,
        "offer": { "nextStep": "review", "available": ["inspect", "review", "restore"] }
    });
    assert_eq!(released_now(), free, "no test has held the settings");

    let test = claims
        .claim(
            &loop_definition.project_id,
            &loop_definition.id,
            "Identify plant",
        )
        .expect("a free loop");
    assert_eq!(
        released_now(),
        json!({
            "message": "Identify plant is running in this engine on this loop. Wait for it to end, then try again.",
            "offer": { "nextStep": "inspect", "available": ["inspect"] }
        }),
        "the test still runs"
    );
    drop(test);

    let mut journal = Journal::open(&folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the loop's journal");
    journal
        .record(JournalEntry::RecordingOwner {
            owner: recording_owner(&loop_definition, "recording-9"),
        })
        .expect("the test's recording is journaled");
    let held = released_now();
    assert!(
        held["message"]
            .as_str()
            .is_some_and(|text| text.contains("Journaled: recording-9.")),
        "the test ended without its restoration confirmed: {held}"
    );
    assert!(held.get("settingsReleased").is_none(), "{held}");

    journal
        .clear("recording-9")
        .expect("the restoration is confirmed and the journal cleared");
    assert_eq!(released_now(), free, "the restoration is confirmed");
    let _ = fs::remove_dir_all(&folder);
}

#[test]
fn a_loop_without_a_tool_has_no_settings_to_release_and_the_action_and_the_answer_have_their_words()
{
    let loop_definition = tank_loop();
    let folder = scratch("released-no-tool");
    let claims = LoopClaims::default();
    let request = released_request(&loop_definition, &folder);
    assert_eq!(
        released(&claims, &loop_definition, &request).expect("answered"),
        json!({}),
        "a loop without a tool answers neither a release nor a reason"
    );
    assert_eq!(
        serde_json::to_value(ParameterWorkAction::Released).expect("the action"),
        json!("released")
    );
    let answer = ParameterWorkAnswer {
        settings_released: Some("Released.".to_string()),
        ..ParameterWorkAnswer::default()
    };
    assert_eq!(
        serde_json::to_value(&answer).expect("the answer"),
        json!({ "settingsReleased": "Released." })
    );
}

/// The updates of an engine's operations, in the order the test receives them.
struct Updates(Mutex<mpsc::Sender<OperationUpdate>>);

impl UpdateSink for Updates {
    fn send(&self, update: OperationUpdate) {
        let _ = self.0.lock().send(update);
    }
}

/// The operations of an engine whose updates the test receives.
fn operations_with_updates() -> (Operations, mpsc::Receiver<OperationUpdate>) {
    let (sender, receiver) = mpsc::channel();
    (
        Operations::new(Arc::new(Updates(Mutex::new(sender)))),
        receiver,
    )
}

/// The update that ends the operation: its result, its failure or its cancellation.
fn final_update(updates: &mpsc::Receiver<OperationUpdate>) -> OperationUpdate {
    loop {
        let update = updates
            .recv_timeout(Duration::from_secs(30))
            .expect("an update of the operation");
        if update.state != OperationState::Running {
            return update;
        }
    }
}

/// The parameter record of a simulated runtime that holds lesson 21's settings: the step 10 and the hold 30 at `active:1`, and, once a
/// write was accepted under `applied`'s transaction, its values at `active:2` with the record before it retained.
fn record_of(applied: Option<&(String, BTreeMap<String, f64>)>) -> Value {
    let before = json!({ "P1.SelectedStepPoints": 10.0, "P1.HoldSeconds": 30.0 });
    match applied {
        None => json!({
            "supports_request_id": true, "transaction_id": null, "program_revision": "program:1",
            "binding_revision": "binding:1", "active_revision": "active:1", "values": before,
            "status": "idle", "previous_revision": null, "previous_values": null, "reason": null
        }),
        Some((transaction, values)) => json!({
            "supports_request_id": true, "transaction_id": transaction, "program_revision": "program:1",
            "binding_revision": "binding:1", "active_revision": "active:2", "values": values,
            "status": "applied", "previous_revision": "active:1", "previous_values": before, "reason": null
        }),
    }
}

/// A simulated runtime (E1.3) whose parameter record holds the settings and takes a write at once (E7.2 to E7.4).
fn settings_runtime() -> FakeRuntime {
    let accepted = Mutex::new(None::<(String, BTreeMap<String, f64>)>);
    FakeRuntime::start(move |request| match request["type"].as_str()? {
        "status" => Some(reply(
            request,
            json!({
                "session_epoch": 7, "simulation_enabled": true, "simulation_only_io": true,
                "simulation_time_scale": 1, "simulation_achieved_time_scale": null,
                "program_revision": "program:1", "tasks": [{ "name": "MainTask", "interval_ms": 100 }],
                "cycle_interval_ms": 100
            }),
        )),
        "control.parameters.status" => Some(reply(request, record_of(accepted.lock().as_ref()))),
        "control.parameters.apply" => {
            let params = &request["params"];
            let transaction = params["transaction_id"].as_str()?.to_string();
            let values: BTreeMap<String, f64> =
                serde_json::from_value(params["values"].clone()).ok()?;
            *accepted.lock() = Some((transaction, values));
            Some(reply(
                request,
                json!({ "status": "pending", "confirmed": false }),
            ))
        }
        _ => Some(refusal(
            request,
            "invalid_parameter_request",
            "Not in this test.",
        )),
    })
}

/// A project folder holding lesson 21's loop, removed when the test ends.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let root = scratch(name);
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), LESSON_21).expect("the loop file");
        Self { root }
    }

    /// The request of `action` on lesson 21's loop, against `runtime`, with the fields of `extra` besides.
    fn request(&self, runtime: &FakeRuntime, action: &str, extra: &Value) -> ParameterWorkRequest {
        let mut request = json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": "simulator",
            "loopId": "control-lesson-21",
            "storageFolder": self.root.join("storage").display().to_string(),
            "connection": runtime.connection(None),
            "action": action
        });
        request
            .as_object_mut()
            .expect("a request is an object")
            .extend(extra.as_object().expect("the fields are an object").clone());
        serde_json::from_value(request).expect("the request decodes")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// C12 against a runtime: the engine inspects the settings, reviews values within their bounds and refuses a value outside them,
// applies the review it gave under its identity and confirms it on the runtime's record, refuses all of it while a project test works on
// the loop, and says the settings are released once the test has ended.
#[test]
fn the_experiment_settings_are_inspected_reviewed_and_applied_on_the_runtime_and_frozen_while_a_test_runs(
) {
    let project = Project::new("settings-runtime");
    let runtime = settings_runtime();
    let (operations, updates) = operations_with_updates();
    let claims = LoopClaims::default();
    let ask = |action: &str, extra: Value| {
        answer(
            &operations,
            &claims,
            project.request(&runtime, action, &extra),
        )
    };

    let inspected = ask("inspect", json!({})).expect("the settings are read");
    assert_eq!(
        inspected["values"][STEP].as_f64(),
        Some(10.0),
        "{inspected}"
    );
    assert_eq!(
        inspected["values"][HOLD].as_f64(),
        Some(30.0),
        "{inspected}"
    );
    assert_eq!(
        inspected["binding"]["parameters"].as_array().map(Vec::len),
        Some(2),
        "the page is given the loop's two settings: {inspected}"
    );

    let above = r#"{"P1.SelectedStepPoints": 12, "P1.HoldSeconds": 41}"#;
    let refused =
        ask("review", json!({ "json": above })).expect_err("41 s is above the hold's maximum");
    assert_eq!(
        refused.message,
        "P1.HoldSeconds is outside its declared bounds after IEC rounding."
    );

    let typed = r#"{"P1.SelectedStepPoints": 12, "P1.HoldSeconds": 35}"#;
    let reviewed =
        ask("review", json!({ "json": typed })).expect("both values are inside their bounds");
    let identity = reviewed["identity"]
        .as_str()
        .expect("the review's identity")
        .to_string();
    assert_eq!(reviewed["values"][HOLD].as_f64(), Some(35.0), "{reviewed}");
    assert_eq!(reviewed["physical"], json!(false));
    assert!(
        runtime.requests_of("control.parameters.apply").is_empty(),
        "a review writes nothing"
    );

    // A project test works on the loop: nothing is reviewed or written.
    let test = claims
        .claim(
            "project:control-lesson-21",
            "control-lesson-21",
            "Identify plant",
        )
        .expect("a free loop");
    let values = settings_of(12.0, 35.0);
    let write = json!({ "review": identity, "values": values });
    assert_eq!(
        ask("confirm", write.clone()).expect_err("the settings are frozen while the test runs"),
        loop_busy_refusal("Identify plant")
    );
    assert_eq!(
        ask("review", json!({ "json": typed })).expect_err("a review is refused too"),
        loop_busy_refusal("Identify plant")
    );
    assert!(
        runtime.requests_of("control.parameters.apply").is_empty(),
        "nothing reached the runtime"
    );
    drop(test);

    // The test has ended: the review the engine gave is applied under its identity.
    let started = ask("confirm", write).expect("the write starts");
    assert!(started["operationId"].is_string(), "{started}");
    let ended = final_update(&updates);
    assert_eq!(ended.state, OperationState::Finished, "{ended:?}");
    let result = ended.result.expect("the confirmed record");
    assert_eq!(result["values"][HOLD].as_f64(), Some(35.0), "{result}");
    assert_eq!(result["observed"]["status"], json!("applied"), "{result}");
    let sent = runtime.requests_of("control.parameters.apply");
    assert_eq!(sent.len(), 1, "the write is sent once");
    assert_eq!(sent[0]["params"]["values"][HOLD].as_f64(), Some(35.0));
    assert_eq!(
        sent[0]["params"]["expected_values"][HOLD].as_f64(),
        Some(30.0)
    );

    // The released settings answer the tool's next offer too (C12 Offer): after the test the record can be inspected, reviewed again,
    // or restored to the values before this write, and the next step is a review.
    let released = ask("released", json!({})).expect("answered");
    assert_eq!(
        released,
        json!({
            "offer": { "available": ["inspect", "review", "restore"], "nextStep": "review" },
            "settingsReleased": SETTINGS_RELEASED
        })
    );
}
