//! Tests of the Apply and Restore methods (`apply_action`): what they refuse at once, before any operation starts and before the runtime
//! is asked anything; the shape of their answers; and the request they decode. The runs themselves are tested with the runtime
//! (`workflow::apply_tests`, `workflow::restore_tests`) and on the real runtime (`tests/control_studio_transactions.rs`).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{
    AnswerKind, ApplicationAnswer, ApplicationRequest, OperationStarted, OperationUpdate,
};

use crate::commands::{ControlStudio, APPLY, KEEP, METHODS, RESTORE};
use crate::operation::UpdateSink;

const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder with the example loop, and the arguments of a reviewed method for it, with `connection` when one is given.
fn example_project(name: &str, connection: Option<Value>) -> (PathBuf, LoopDefinition, Value) {
    let root = std::env::temp_dir().join(format!(
        "trust-control-apply-action-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("project");
    fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
    let example = LoopDefinition::decode(EXAMPLE).expect("example");
    let mut request = json!({
        "projectRoot": root.display().to_string(),
        "runtimeId": example.runtime_id,
        "loopId": example.id,
        "storageFolder": root.join("storage").display().to_string(),
    });
    if let Some(connection) = connection {
        request["connection"] = connection;
    }
    (root, example, request)
}

#[test]
fn the_reviewed_methods_are_the_engines_and_a_malformed_request_is_a_failure() {
    for method in [APPLY, RESTORE, KEEP] {
        assert!(METHODS.contains(&method), "the engine answers {method}");
    }
    let studio = ControlStudio::new(Arc::new(Ignore));
    for method in [APPLY, RESTORE] {
        let malformed = studio
            .execute(method, &[json!({ "projectRoot": "/x" })])
            .expect("answered");
        assert!(
            malformed["failure"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("invalid arguments")),
            "{malformed}"
        );
        assert!(malformed.get("result").is_none(), "{malformed}");
    }
}

#[test]
fn a_request_decodes_its_review_identity_and_refuses_an_unknown_field() {
    let request: ApplicationRequest = serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "tank",
        "storageFolder": "/journal",
        "review": "sha256:abc",
    }))
    .expect("decoded");
    assert_eq!(request.review.as_deref(), Some("sha256:abc"));
    assert!(
        !request.physical_approved,
        "a physical target is not approved unless the reviewed approval says so (A2.5)"
    );
    assert!(request.connection.is_none());
    // `confirm` is a retired field (the recovery's Restore is confirmed by the review's own identity): it is refused like any other.
    for field in ["approve", "confirm"] {
        let unknown = serde_json::from_value::<ApplicationRequest>(json!({
            "projectRoot": "/project",
            "runtimeId": "simulator",
            "loopId": "tank",
            "storageFolder": "/journal",
            field: true,
        }));
        assert!(unknown.is_err(), "the unknown field `{field}` is refused");
    }
}

#[test]
fn an_apply_without_a_runtime_connection_is_refused_and_writes_nothing() {
    let (root, _example, request) = example_project("no-connection", None);
    let studio = ControlStudio::new(Arc::new(Ignore));
    let refused = studio
        .execute(APPLY, std::slice::from_ref(&request))
        .expect("answered");
    assert!(refused.get("failure").is_some(), "{refused}");
    assert!(refused.get("result").is_none(), "{refused}");
    assert_eq!(
        refused["failure"]["nextAction"], "connect-runtime",
        "{refused}"
    );
    assert!(
        !root.join("control-studio").exists(),
        "a refusal writes nothing"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_restore_is_refused_at_once_when_no_apply_has_applied_its_settings() {
    let (root, _example, request) = example_project(
        "restore-none",
        Some(json!({ "endpoint": "tcp://127.0.0.1:1" })),
    );
    let studio = ControlStudio::new(Arc::new(Ignore));
    let refused = studio.execute(RESTORE, &[request]).expect("answered");
    assert!(
        refused["failure"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Restore previous settings is not available")),
        "{refused}"
    );
    assert!(refused.get("result").is_none(), "{refused}");
    assert_eq!(refused["failure"]["nextAction"], "none", "{refused}");
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn an_apply_refuses_a_loop_that_an_operation_of_this_engine_holds() {
    let (root, example, mut request) =
        example_project("busy", Some(json!({ "endpoint": "tcp://127.0.0.1:1" })));
    request["review"] = json!("sha256:the-review");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let _held = studio
        .loop_claims()
        .claim(&example.project_id, &example.id, "Validate")
        .expect("a free loop");
    let refused = studio.execute(APPLY, &[request]).expect("answered");
    assert!(
        refused["failure"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("is running in this engine")),
        "{refused}"
    );
    assert!(refused.get("result").is_none(), "{refused}");
    fs::remove_dir_all(&root).expect("clean up");
}

/// §0.1a: a started Apply, Restore or Keep is answered by its kind and the operation's identifier, as every started operation is, and
/// nothing else. The wrong behaviour caught: a started operation that the page can tell from a review only by the field it carries.
#[test]
fn a_started_operation_is_answered_by_its_kind_and_identifier_only() {
    let started = serde_json::to_value(ApplicationAnswer::Started(OperationStarted {
        kind: AnswerKind::new(),
        operation_id: "operation-1".to_string(),
    }))
    .expect("written");
    assert_eq!(
        started,
        json!({ "kind": "operationStarted", "operationId": "operation-1" })
    );
}
