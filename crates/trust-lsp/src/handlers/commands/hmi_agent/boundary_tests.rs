//! Ports of `editors/vscode/src/test/suite/hmi-agent-boundary.test.ts` (suite "HMI agent request
//! boundary"): the same envelope, workspace and refusal expectations, with the operation counted.
//! Specification 43 §4.
//!
//! Not ported: the cyclic, `Infinity` and `undefined` inputs. A `serde_json::Value` cannot hold
//! them; the transport refuses such text when it parses the request, before this boundary runs.
//! The malformed-edit schema cases of the same TypeScript file belong to the schema tests.

use std::cell::Cell;
use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{invoke, Success, MAX_REQUEST_BYTES};

/// The workspace this connection serves.
const WORKSPACE: &str = "window-project-1";

/// The request the TypeScript suite calls `valid`.
fn valid() -> Value {
    json!({"contractVersion": 1, "workspaceId": WORKSPACE, "input": {"page": "overview"}})
}

/// The answer of an operation that succeeds, in the envelope.
fn success_answer() -> Value {
    json!({
        "ok": true,
        "data": {"title": "Overview"},
        "revisions": {"page": "hash-1"},
        "warnings": [],
    })
}

/// The operation's success, as the TypeScript suite's `success` constant.
fn success() -> Success {
    Success {
        data: json!({"title": "Overview"}),
        revisions: BTreeMap::from([("page".to_owned(), "hash-1".to_owned())]),
        warnings: Vec::new(),
    }
}

/// Runs a request with an operation that counts its calls, and returns the answer and the count.
fn run(request: &Value, cancelled: bool) -> (Value, u32) {
    let calls = Cell::new(0_u32);
    let answer = invoke(request, WORKSPACE, cancelled, |_input| {
        calls.set(calls.get() + 1);
        Ok(success())
    });
    (answer, calls.get())
}

/// Asserts that a request is refused with the code, and that no operation ran.
fn assert_refused(request: &Value, code: &str, cancelled: bool) {
    let (answer, calls) = run(request, cancelled);
    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert_eq!(answer["code"], json!(code), "{answer}");
    assert_eq!(
        calls, 0,
        "an invalid request must not reach the authoring operation"
    );
}

#[test]
fn forwards_only_validated_input_with_the_workspace_authority() {
    let received = Cell::new(false);
    let answer = invoke(&valid(), WORKSPACE, false, |input| {
        assert_eq!(Value::Object(input.clone()), json!({"page": "overview"}));
        received.set(true);
        Ok(success())
    });
    assert!(received.get(), "the operation receives the validated input");
    assert_eq!(answer, success_answer());
}

#[test]
fn an_unsupported_contract_version_is_refused_before_an_operation() {
    let mut request = valid();
    request["contractVersion"] = json!(2);
    assert_refused(&request, "UNSUPPORTED_VERSION", false);
}

#[test]
fn a_caller_cannot_choose_another_workspace() {
    let mut request = valid();
    request["workspaceId"] = json!("other-project");
    assert_refused(&request, "PROJECT_UNAVAILABLE", false);
}

#[test]
fn a_cancelled_request_never_reaches_an_operation() {
    assert_refused(&valid(), "CANCELLED", true);
}

#[test]
fn invalid_envelopes_and_non_object_inputs_are_rejected() {
    let mut extra = valid();
    extra["extra"] = json!(true);
    let mut array_input = valid();
    array_input["input"] = json!([]);
    let mut null_input = valid();
    null_input["input"] = Value::Null;
    for request in [
        json!(null),
        json!([]),
        json!({}),
        array_input,
        null_input,
        extra,
    ] {
        assert_refused(&request, "INVALID_ARGUMENT", false);
    }
}

#[test]
fn oversized_requests_are_rejected_before_an_operation() {
    let mut request = valid();
    request["input"] = json!({"text": "x".repeat(8 * 1024 * 1024)});
    assert_refused(&request, "LIMIT_EXCEEDED", false);
}

#[test]
fn a_request_of_exactly_the_limit_is_accepted_and_one_byte_more_is_refused() {
    let mut request = valid();
    request["input"] = json!({"text": ""});
    let overhead = request.to_string().len();
    let fill = MAX_REQUEST_BYTES - overhead;
    request["input"] = json!({"text": "x".repeat(fill)});
    assert_eq!(request.to_string().len(), MAX_REQUEST_BYTES);
    let (answer, calls) = run(&request, false);
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(calls, 1);
    request["input"] = json!({"text": "x".repeat(fill + 1)});
    assert_refused(&request, "LIMIT_EXCEEDED", false);
}

#[test]
fn a_refusal_carries_the_shared_envelope_without_details() {
    let mut request = valid();
    request["contractVersion"] = json!(2);
    let (answer, _) = run(&request, false);
    assert_eq!(
        answer.as_object().map(|object| object.len()),
        Some(4),
        "{answer}"
    );
    assert_eq!(answer["retryable"], json!(false));
}
