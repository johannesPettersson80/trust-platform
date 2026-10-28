//! Tests of the records that Apply and Restore answer and of their review identity (specification 35, §0.1b Confirmation, B6.1
//! to B6.5; annex E11.8). No TypeScript test is ported: the TypeScript kept its reviews in memory, without an identity.

use std::collections::BTreeSet;

use serde_json::{json, Map};

use super::*;
use crate::commissioning::commissioning_result::CommissioningStatus;
use crate::commissioning::parameter_status::RecordState;
use crate::loop_definition::Range;
use crate::recording::fixtures;
use crate::validation::commissioning_assessment::CommissioningCriteria;

fn values(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), *value))
        .collect()
}

/// A parameter record as the runtime reports it (E7.2): two gains in force, and the previous set retained.
fn status() -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("transaction-1".to_owned()),
        program_revision: "program:1".to_owned(),
        binding_revision: "binding:1".to_owned(),
        active_revision: "active:1".to_owned(),
        values: values(&[("Main.Parameters.Kp", 1.0), ("Main.Parameters.Ki", 0.1)]),
        status: RecordState::Applied,
        previous_revision: Some("previous:1".to_owned()),
        previous_values: Some(values(&[
            ("Main.Parameters.Kp", 0.5),
            ("Main.Parameters.Ki", 0.05),
        ])),
        reason: None,
    }
}

/// The candidate an Apply writes.
fn candidate() -> BTreeMap<String, f64> {
    values(&[("Main.Parameters.Kp", 2.0), ("Main.Parameters.Ki", 0.5)])
}

/// The values from before Apply that a Restore writes back: the checkpoint's origin values.
fn origin() -> BTreeMap<String, f64> {
    values(&[("Main.Parameters.Kp", 0.5), ("Main.Parameters.Ki", 0.05)])
}

/// The identity of an Apply with these facts.
fn apply_identity(
    status: &ParameterStatus,
    found_mode: Option<FoundMode>,
    written: &BTreeMap<String, f64>,
    transfer_binding_revision: &str,
    validation_identity: &str,
) -> String {
    review_identity(&apply_identity_parts(
        status,
        found_mode,
        written,
        transfer_binding_revision,
        validation_identity,
    ))
}

/// The identity of a Restore with these facts.
fn restore_identity(
    status: &ParameterStatus,
    found_mode: Option<FoundMode>,
    written: &BTreeMap<String, f64>,
    transfer_binding_revision: &str,
    checkpoint_id: &str,
) -> String {
    review_identity(&restore_identity_parts(
        status,
        found_mode,
        written,
        transfer_binding_revision,
        checkpoint_id,
    ))
}

/// The keys of a JSON object; none for any other value.
fn keys(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// A commissioning result whose requirement failed while the settings stay applied (B6.3).
fn failed_result() -> CommissioningResult {
    CommissioningResult {
        status: CommissioningStatus::Failed,
        reason: "The response left the declared band.".to_owned(),
        recording: None,
        transfers: Vec::new(),
        recovery_confirmed: false,
        maximum_error: Some(0.3),
        target: 38.0,
        parameters: Some(status()),
        model_revision: "sha256:model".to_owned(),
        channels: Vec::new(),
        criteria: CommissioningCriteria {
            target: 38.0,
            duration_seconds: 60.0,
            settling_time_seconds: 30.0,
            maximum_error: 0.2,
            task_period_ms: 100.0,
            process_range: Range {
                minimum: 0.0,
                maximum: 80.0,
            },
            output_limits: Range {
                minimum: 0.0,
                maximum: 100.0,
            },
            switching: None,
        },
        gain_recovery: None,
        failure: None,
        found_mode: Some(FoundMode::Auto),
    }
}

/// §0.1b Confirmation: the identity is `sha256:` and the lowercase hex of the SHA-256 digest of the canonical JSON, whose keys
/// are sorted at every level, so parts built in another order give the same identity, and the same parts always give it. The
/// expected digest was computed independently, as the SHA-256 of the text `{"a":"x","b":{"c":[2,1],"d":1}}`.
#[test]
fn the_identity_is_the_sha256_of_the_canonical_json_whatever_the_key_order() {
    let expected = "sha256:9485795dd2e31a4134702cc8889e292fa1f69e268326c2988ca687fbc981ea48";
    let built = json!({ "b": { "d": 1, "c": [2, 1] }, "a": "x" });
    assert_eq!(review_identity(&built), expected);

    let mut inner = Map::new();
    inner.insert("c".to_owned(), json!([2, 1]));
    inner.insert("d".to_owned(), json!(1));
    let mut outer = Map::new();
    outer.insert("a".to_owned(), json!("x"));
    outer.insert("b".to_owned(), Value::Object(inner));
    assert_eq!(
        review_identity(&Value::Object(outer)),
        expected,
        "the same parts, built in another order"
    );
    assert_ne!(
        review_identity(&json!({ "a": "x", "b": { "c": [1, 2], "d": 1 } })),
        expected,
        "an array keeps its order"
    );
}

/// §0.1b Confirmation: an Apply's parts are the record's program, binding and active revisions, the mode found, the values to
/// write, the transfer binding revision and the study's validation identity, under exactly these keys; an unknown mode is
/// `null`.
#[test]
fn the_apply_parts_hold_exactly_what_the_confirmation_approves() {
    let parts = apply_identity_parts(&status(), None, &candidate(), "transfer:1", "validation:1");
    assert_eq!(
        keys(&parts),
        BTreeSet::from([
            "activeRevision",
            "bindingRevision",
            "foundMode",
            "programRevision",
            "transferBindingRevision",
            "validationIdentity",
            "values",
        ])
    );
    assert_eq!(parts["programRevision"], "program:1");
    assert_eq!(parts["bindingRevision"], "binding:1");
    assert_eq!(parts["activeRevision"], "active:1");
    assert_eq!(parts["foundMode"], Value::Null);
    assert_eq!(
        keys(&parts["values"]),
        BTreeSet::from(["Main.Parameters.Ki", "Main.Parameters.Kp"])
    );
    assert_eq!(parts["values"]["Main.Parameters.Kp"], 2.0);
    assert_eq!(parts["values"]["Main.Parameters.Ki"], 0.5);
    assert_eq!(parts["transferBindingRevision"], "transfer:1");
    assert_eq!(parts["validationIdentity"], "validation:1");

    let found = apply_identity_parts(
        &status(),
        Some(FoundMode::Auto),
        &candidate(),
        "transfer:1",
        "validation:1",
    );
    assert_eq!(found["foundMode"], "auto");
}

/// §0.1b Confirmation: a change of any fact an Apply's confirmation approves (each revision, the mode, one value to write, the
/// transfer binding revision or the study's validation) gives another identity, so the confirmation is refused.
#[test]
fn an_apply_identity_changes_with_each_fact_its_confirmation_approves() {
    let auto = Some(FoundMode::Auto);
    let base = apply_identity(&status(), auto, &candidate(), "transfer:1", "validation:1");
    let mut program = status();
    program.program_revision = "program:2".to_owned();
    let mut binding = status();
    binding.binding_revision = "binding:2".to_owned();
    let mut active = status();
    active.active_revision = "active:2".to_owned();
    let mut one_value = candidate();
    one_value.insert("Main.Parameters.Ki".to_owned(), 0.6);
    let changed = [
        (
            "program revision",
            apply_identity(&program, auto, &candidate(), "transfer:1", "validation:1"),
        ),
        (
            "binding revision",
            apply_identity(&binding, auto, &candidate(), "transfer:1", "validation:1"),
        ),
        (
            "active revision",
            apply_identity(&active, auto, &candidate(), "transfer:1", "validation:1"),
        ),
        (
            "mode found in Manual",
            apply_identity(
                &status(),
                Some(FoundMode::Manual),
                &candidate(),
                "transfer:1",
                "validation:1",
            ),
        ),
        (
            "unknown mode",
            apply_identity(&status(), None, &candidate(), "transfer:1", "validation:1"),
        ),
        (
            "value to write",
            apply_identity(&status(), auto, &one_value, "transfer:1", "validation:1"),
        ),
        (
            "transfer binding revision",
            apply_identity(&status(), auto, &candidate(), "transfer:2", "validation:1"),
        ),
        (
            "validation identity",
            apply_identity(&status(), auto, &candidate(), "transfer:1", "validation:2"),
        ),
    ];
    for (fact, identity) in &changed {
        assert_ne!(identity, &base, "a changed {fact} gives another identity");
    }
    let distinct: BTreeSet<&str> = changed
        .iter()
        .map(|(_, identity)| identity.as_str())
        .chain([base.as_str()])
        .collect();
    assert_eq!(
        distinct.len(),
        changed.len() + 1,
        "each fact moves the identity on its own"
    );
}

/// §0.1b Confirmation and B6.4: a Restore's parts are the record's program, binding and active revisions, the mode found, the
/// values it writes back (the checkpoint's origin values, E7.5), the transfer binding revision and the checkpoint it starts
/// from (A8.5), under exactly these keys; an unknown mode is `null`.
#[test]
fn the_restore_parts_hold_exactly_what_the_confirmation_approves() {
    let parts = restore_identity_parts(
        &status(),
        Some(FoundMode::Manual),
        &origin(),
        "transfer:1",
        "checkpoint:1",
    );
    assert_eq!(
        keys(&parts),
        BTreeSet::from([
            "activeRevision",
            "bindingRevision",
            "checkpointId",
            "foundMode",
            "programRevision",
            "transferBindingRevision",
            "values",
        ])
    );
    assert_eq!(parts["programRevision"], "program:1");
    assert_eq!(parts["bindingRevision"], "binding:1");
    assert_eq!(parts["activeRevision"], "active:1");
    assert_eq!(parts["foundMode"], "manual");
    assert_eq!(
        keys(&parts["values"]),
        BTreeSet::from(["Main.Parameters.Ki", "Main.Parameters.Kp"])
    );
    assert_eq!(parts["values"]["Main.Parameters.Kp"], 0.5);
    assert_eq!(parts["values"]["Main.Parameters.Ki"], 0.05);
    assert_eq!(parts["transferBindingRevision"], "transfer:1");
    assert_eq!(parts["checkpointId"], "checkpoint:1");

    let unknown = restore_identity_parts(&status(), None, &origin(), "transfer:1", "checkpoint:1");
    assert_eq!(unknown["foundMode"], Value::Null);
}

/// §0.1b Confirmation and B6.4: a change of any fact a Restore's confirmation approves (each revision, the mode, one value it
/// writes back, the transfer binding revision or the checkpoint) gives another identity, so the confirmation is refused.
#[test]
fn a_restore_identity_changes_with_each_fact_its_confirmation_approves() {
    let auto = Some(FoundMode::Auto);
    let base = restore_identity(&status(), auto, &origin(), "transfer:1", "checkpoint:1");
    let mut program = status();
    program.program_revision = "program:2".to_owned();
    let mut binding = status();
    binding.binding_revision = "binding:2".to_owned();
    let mut active = status();
    active.active_revision = "active:2".to_owned();
    let mut one_value = origin();
    one_value.insert("Main.Parameters.Ki".to_owned(), 0.06);
    let changed = [
        (
            "program revision",
            restore_identity(&program, auto, &origin(), "transfer:1", "checkpoint:1"),
        ),
        (
            "binding revision",
            restore_identity(&binding, auto, &origin(), "transfer:1", "checkpoint:1"),
        ),
        (
            "active revision",
            restore_identity(&active, auto, &origin(), "transfer:1", "checkpoint:1"),
        ),
        (
            "mode found in Manual",
            restore_identity(
                &status(),
                Some(FoundMode::Manual),
                &origin(),
                "transfer:1",
                "checkpoint:1",
            ),
        ),
        (
            "unknown mode",
            restore_identity(&status(), None, &origin(), "transfer:1", "checkpoint:1"),
        ),
        (
            "value to write back",
            restore_identity(&status(), auto, &one_value, "transfer:1", "checkpoint:1"),
        ),
        (
            "transfer binding revision",
            restore_identity(&status(), auto, &origin(), "transfer:2", "checkpoint:1"),
        ),
        (
            "checkpoint",
            restore_identity(&status(), auto, &origin(), "transfer:1", "checkpoint:2"),
        ),
    ];
    for (fact, identity) in &changed {
        assert_ne!(identity, &base, "a changed {fact} gives another identity");
    }
    let distinct: BTreeSet<&str> = changed
        .iter()
        .map(|(_, identity)| identity.as_str())
        .chain([base.as_str()])
        .collect();
    assert_eq!(
        distinct.len(),
        changed.len() + 1,
        "each fact moves the identity on its own"
    );
}

/// §0.1b: an Apply and a Restore approve different things, so even the same runtime facts, values and transfer binding give
/// them different identities: an Apply names the study's validation and a Restore its checkpoint, and the review of one never
/// confirms the other.
#[test]
fn an_apply_and_a_restore_of_the_same_runtime_have_different_identities() {
    let apply = apply_identity_parts(
        &status(),
        Some(FoundMode::Auto),
        &origin(),
        "transfer:1",
        "validation:1",
    );
    let restore = restore_identity_parts(
        &status(),
        Some(FoundMode::Auto),
        &origin(),
        "transfer:1",
        "checkpoint:1",
    );
    for fact in [
        "programRevision",
        "bindingRevision",
        "activeRevision",
        "foundMode",
        "values",
        "transferBindingRevision",
    ] {
        assert_eq!(apply[fact], restore[fact], "both hold the same {fact}");
    }
    let apply_keys = keys(&apply);
    let restore_keys = keys(&restore);
    let own: BTreeSet<&str> = apply_keys
        .symmetric_difference(&restore_keys)
        .copied()
        .collect();
    assert_eq!(
        own,
        BTreeSet::from(["checkpointId", "validationIdentity"]),
        "only what each one approves of its own differs"
    );
    assert_ne!(review_identity(&apply), review_identity(&restore));
}

/// §0.1b `apply` and B6.1: the review shows its kind (§0.1a), the action, its identity, the mode, each parameter's running and next
/// value, the description, whether the target is physical and the validated operating point, under exactly these `camelCase` keys; an
/// unknown mode or a missing operating point is `null`, never left out.
#[test]
#[allow(clippy::too_many_lines)] // The review's keys are written out in full by hand, so a renamed or a missing key is caught.
fn the_review_serializes_with_exactly_its_camel_case_keys() {
    let review = ApplicationReview {
        kind: AnswerKind::new(),
        action: ReviewedAction::Restore,
        identity: "sha256:review".to_owned(),
        found_mode: None,
        parameters: vec![ReviewedParameter {
            symbol: "Main.Parameters.Kp".to_owned(),
            unit: "%/%".to_owned(),
            running: 2.0,
            next: 0.5,
        }],
        description: "Restore the previous settings.".to_owned(),
        physical: false,
        confirmation: ReviewConfirmation::Press,
        operating_point: None,
        approval: ApprovalBlock {
            loop_id: "Main.Heater".to_owned(),
            signals: vec![ApprovedSignal {
                symbol: "Main.Parameters.Kp".to_owned(),
                unit: "%/%".to_owned(),
                running: 2.0,
                next: 0.5,
                minimum: 0.0,
                maximum: 10.0,
            }],
            duration_seconds: None,
            stop_conditions: vec!["A step the runtime does not confirm.".to_owned()],
            restoration: "The record goes back.".to_owned(),
        },
    };
    let value = serde_json::to_value(&review).expect("the review serializes");
    assert_eq!(
        keys(&value),
        BTreeSet::from([
            "action",
            "approval",
            "confirmation",
            "description",
            "foundMode",
            "identity",
            "kind",
            "operatingPoint",
            "parameters",
            "physical",
        ])
    );
    assert_eq!(value["kind"], "applicationReview");
    assert_eq!(value["action"], "restore");
    assert_eq!(value["identity"], "sha256:review");
    assert_eq!(value["foundMode"], Value::Null);
    assert_eq!(value["description"], "Restore the previous settings.");
    assert_eq!(value["physical"], json!(false));
    assert_eq!(value["confirmation"], "press");
    assert_eq!(value["operatingPoint"], Value::Null);
    let parameter = &value["parameters"][0];
    assert_eq!(
        keys(parameter),
        BTreeSet::from(["next", "running", "symbol", "unit"])
    );
    assert_eq!(parameter["symbol"], "Main.Parameters.Kp");
    assert_eq!(parameter["unit"], "%/%");
    assert_eq!(parameter["running"], 2.0);
    assert_eq!(parameter["next"], 0.5);
    assert_eq!(
        keys(&value["approval"]),
        BTreeSet::from([
            "durationSeconds",
            "loopId",
            "restoration",
            "signals",
            "stopConditions",
        ])
    );
    assert_eq!(value["approval"]["durationSeconds"], Value::Null);
    assert_eq!(value["approval"]["loopId"], "Main.Heater");
    assert_eq!(
        keys(&value["approval"]["signals"][0]),
        BTreeSet::from(["maximum", "minimum", "next", "running", "symbol", "unit"])
    );
    assert_eq!(value["approval"]["signals"][0]["maximum"], 10.0);

    let apply = ApplicationReview {
        action: ReviewedAction::Apply,
        found_mode: Some(FoundMode::Auto),
        physical: true,
        confirmation: ReviewConfirmation::Approval,
        operating_point: Some(ReviewedOperatingPoint {
            inputs: vec![40.0],
            outputs: vec![2.0],
        }),
        ..review
    };
    let value = serde_json::to_value(&apply).expect("the review serializes");
    assert_eq!(value["action"], "apply");
    assert_eq!(value["foundMode"], "auto");
    assert_eq!(value["physical"], json!(true));
    assert_eq!(value["confirmation"], "approval");
    assert_eq!(
        keys(&value["operatingPoint"]),
        BTreeSet::from(["inputs", "outputs"])
    );
    assert_eq!(value["operatingPoint"]["inputs"][0], 40.0);
    assert_eq!(value["operatingPoint"]["outputs"][0], 2.0);
}

/// B6.3, B6.4, B6.5 and A8.1: Apply's final result carries its kind (§0.1a), the commissioning result, what the engineer may do next,
/// and the checkpoint and validation it was applied under; Restore's carries its kind, whether the runtime confirmed it, why, the mode,
/// the parameter record, whose fields keep the names of annex E7.2, and the recording it kept as evidence, named only when one was
/// held (E11.6).
#[test]
fn the_outcomes_serialize_with_exactly_their_camel_case_keys() {
    let applied = ApplyOutcome {
        kind: AnswerKind::new(),
        result: failed_result(),
        restored: false,
        restore_available: true,
        keep_available: true,
        apply_available: false,
        primary_action: Some(PrimaryAction::Keep),
        stages: None,
        checkpoint_id: "checkpoint:1".to_owned(),
        validation_identity: "validation:1".to_owned(),
    };
    let value = serde_json::to_value(&applied).expect("the outcome serializes");
    assert_eq!(
        keys(&value),
        BTreeSet::from([
            "applyAvailable",
            "checkpointId",
            "keepAvailable",
            "kind",
            "primaryAction",
            "restoreAvailable",
            "restored",
            "result",
            "validationIdentity",
        ]),
        "the workflow's own outcome names no stages: the method adds them"
    );
    assert_eq!(value["kind"], "applyOutcome");
    assert_eq!(value["primaryAction"], "keep");
    assert_eq!(value["result"]["status"], "failed");
    assert_eq!(value["result"]["foundMode"], "auto");
    assert_eq!(value["restored"], json!(false));
    assert_eq!(value["restoreAvailable"], json!(true));
    assert_eq!(value["keepAvailable"], json!(true));
    assert_eq!(value["applyAvailable"], json!(false));
    assert_eq!(value["checkpointId"], "checkpoint:1");
    assert_eq!(value["validationIdentity"], "validation:1");

    let confirmed = RestoreOutcome {
        kind: AnswerKind::new(),
        restored: true,
        reason: "The previous settings are restored.".to_owned(),
        mode: Some(FoundMode::Auto),
        parameters: status(),
        recording: None,
        restore_available: false,
        keep_available: false,
        apply_available: false,
        primary_action: None,
        stages: None,
    };
    let value = serde_json::to_value(&confirmed).expect("the outcome serializes");
    assert_eq!(
        keys(&value),
        BTreeSet::from([
            "applyAvailable",
            "keepAvailable",
            "kind",
            "mode",
            "parameters",
            "reason",
            "restoreAvailable",
            "restored",
        ]),
        "no recording was held, so none is named, and nothing offered names no position"
    );
    assert_eq!(value["kind"], "restoreOutcome");
    assert_eq!(value["restored"], json!(true));
    assert_eq!(value["restoreAvailable"], json!(false));
    assert_eq!(value["keepAvailable"], json!(false));
    assert_eq!(value["applyAvailable"], json!(false));
    assert_eq!(value["reason"], "The previous settings are restored.");
    assert_eq!(value["mode"], "auto");
    assert_eq!(value["parameters"]["program_revision"], "program:1");
    assert_eq!(value["parameters"]["active_revision"], "active:1");
    assert_eq!(value["parameters"]["previous_revision"], "previous:1");

    let kept = RestoreOutcome {
        recording: Some(fixtures::run(
            vec![fixtures::binding("LevelPV", "%IL0", "LREAL")],
            Vec::new(),
        )),
        ..confirmed
    };
    let value = serde_json::to_value(&kept).expect("the outcome serializes");
    assert_eq!(
        keys(&value),
        BTreeSet::from([
            "applyAvailable",
            "keepAvailable",
            "kind",
            "mode",
            "parameters",
            "reason",
            "recording",
            "restoreAvailable",
            "restored",
        ])
    );
    assert_eq!(value["recording"]["recording_id"], "rec-fixture");
}

/// A2.4, A2.5, B6.5 and B9.3: a write to a physical target is confirmed with the engineer's approval and one to a simulated target
/// by the press of the action, which the review says in a word; a Keep in project is confirmed with its own button on any target.
#[test]
fn a_review_says_how_it_is_confirmed_by_the_kind_of_target() {
    assert_eq!(
        ReviewConfirmation::of_target(false),
        ReviewConfirmation::Press
    );
    assert_eq!(
        ReviewConfirmation::of_target(true),
        ReviewConfirmation::Approval
    );
    let words: Vec<Value> = [
        ReviewConfirmation::Press,
        ReviewConfirmation::Approval,
        ReviewConfirmation::Explicit,
    ]
    .iter()
    .map(|confirmation| serde_json::to_value(confirmation).expect("the word serializes"))
    .collect();
    assert_eq!(
        words,
        [json!("press"), json!("approval"), json!("explicit")]
    );
}

/// A2.5 and B9.3: the approval names each signal the review writes with its declared limits, the observation's duration, the stop
/// conditions and the restoration. A Restore records nothing, so it has no duration; a signal the loop declares no range for is named
/// without limits.
#[test]
fn an_approval_names_each_written_signal_with_its_limits_and_the_stop_conditions() {
    let loop_definition = crate::commissioning::fixture_loops::parallel_pid_loop();
    let first = loop_definition
        .parameter_binding
        .as_ref()
        .and_then(|binding| binding.parameters.first())
        .expect("the fixture loop writes a parameter record");
    let written = vec![ReviewedParameter {
        symbol: first.symbol.clone(),
        unit: first.unit.clone(),
        running: 2.0,
        next: 3.0,
    }];

    let apply = approval_block(
        &loop_definition,
        ReviewedAction::Apply,
        &written,
        Some(12.5),
    );
    assert_eq!(apply.loop_id, loop_definition.id);
    assert_eq!(apply.duration_seconds, Some(12.5));
    assert_eq!(apply.signals.len(), 1);
    let signal = &apply.signals[0];
    assert_eq!(signal.symbol, first.symbol);
    assert_eq!((signal.running, signal.next), (2.0, 3.0));
    assert_eq!(signal.minimum, first.minimum);
    assert_eq!(signal.maximum, first.maximum);
    assert!(apply
        .stop_conditions
        .iter()
        .any(|text| text.contains("permitted range")));
    assert!(apply.restoration.starts_with("Restore previous settings"));

    let restore = approval_block(&loop_definition, ReviewedAction::Restore, &written, None);
    assert_eq!(restore.duration_seconds, None);
    assert_eq!(restore.stop_conditions.len(), 1);
    assert!(restore.restoration.starts_with("The parameter record"));

    let undeclared = vec![ReviewedParameter {
        symbol: "Main.NotDeclared".to_owned(),
        unit: String::new(),
        running: 1.0,
        next: 1.0,
    }];
    let approval = approval_block(&loop_definition, ReviewedAction::Apply, &undeclared, None);
    assert!(approval.signals[0].minimum.is_nan());
    assert!(approval.signals[0].maximum.is_nan());
}

/// §0.1b: the action a review is for serializes to the word of layer 1's action (`ApplicationAction::word`), and only those two
/// words decode.
#[test]
fn a_reviewed_action_is_the_answered_word_of_the_application_action() {
    assert_eq!(
        ReviewedAction::from(ApplicationAction::Apply),
        ReviewedAction::Apply
    );
    assert_eq!(
        ReviewedAction::from(ApplicationAction::Restore),
        ReviewedAction::Restore
    );
    for action in [ApplicationAction::Apply, ApplicationAction::Restore] {
        let reviewed = ReviewedAction::from(action);
        assert_eq!(
            serde_json::to_value(reviewed).expect("the action serializes"),
            action.word()
        );
        assert_eq!(
            crate::json::decode_value::<ReviewedAction>(json!(action.word()))
                .expect("the word decodes"),
            reviewed
        );
    }
    assert!(crate::json::decode_value::<ReviewedAction>(json!("rollback")).is_err());
}
