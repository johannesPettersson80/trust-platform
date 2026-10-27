//! One control request and one probe against a fake runtime (runtime protocol annex E0.1;
//! specification 11 §6.9.6).

use serde_json::{json, Value};

use super::super::fake_runtime::{closed_endpoint, FakeRuntime};
use super::answer;

fn request(endpoint: &str, token: Option<&str>, kind: &str, params: Value) -> Value {
    answer(json!({
        "action": "request",
        "endpoint": endpoint,
        "authToken": token,
        "type": kind,
        "params": params,
        "timeoutMs": 1000,
    }))
    .expect("an answer")
}

fn refusing(answer_line: Value) -> FakeRuntime {
    FakeRuntime::start(move |request| {
        let mut line = answer_line.clone();
        line["id"] = request["id"].clone();
        Some(line.to_string())
    })
}

/// One JSON line goes out with the type, the params and the token, and the runtime's result comes
/// back. Catches a request that drops the token or the params, and a result that is not the
/// runtime's.
#[test]
fn a_request_sends_one_line_and_answers_the_runtimes_result() {
    let runtime = FakeRuntime::answering(json!({ "state": "running" }));
    let answer = request(
        &runtime.endpoint,
        Some("auth-token"),
        "status",
        json!({ "verbose": true }),
    );
    assert_eq!(
        answer,
        json!({ "ok": true, "outcome": "result", "result": { "state": "running" } })
    );
    let requests = runtime.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["type"], "status");
    assert_eq!(requests[0]["auth"], "auth-token");
    assert_eq!(requests[0]["params"], json!({ "verbose": true }));
}

/// A blank token is never sent as a credential. Catches a request line with `"auth": ""`, which a
/// runtime that needs no token refuses as invalid.
#[test]
fn a_blank_token_is_not_sent() {
    let runtime = FakeRuntime::answering(json!({}));
    request(&runtime.endpoint, Some("  "), "status", Value::Null);
    assert!(
        runtime.requests()[0].get("auth").is_none(),
        "{:?}",
        runtime.requests()
    );
}

/// The credential failure is the code's (specification 11 §6.9.6): `invalid_auth_token` is a
/// rejected token, `missing_auth_token` a missing one. Catches the old reading of the refusal's
/// text.
#[test]
fn credential_failures_are_named_by_their_codes() {
    for (code, kind) in [
        ("invalid_auth_token", "rejected"),
        ("missing_auth_token", "missing"),
    ] {
        let runtime = refusing(json!({ "ok": false, "error_code": code, "error": "auth problem" }));
        let answer = request(&runtime.endpoint, None, "status", Value::Null);
        assert_eq!(answer["outcome"], "refused", "{code}");
        assert_eq!(answer["code"], code);
        assert_eq!(answer["authFailure"], kind, "{code}");
        assert_eq!(answer["error"], "auth problem");
    }
}

/// A refusal whose text speaks of permission or authorization but carries no credential code is no
/// credential failure. Catches the text heuristics the extension used before 26 October 2026,
/// which sent the user to set a token for a role refusal.
#[test]
fn a_refusal_text_is_never_read_as_a_credential_failure() {
    let runtime = refusing(json!({
        "ok": false,
        "error_code": "forbidden",
        "error": "permission denied: unauthorized role viewer",
    }));
    let answer = request(&runtime.endpoint, None, "io.write", json!({}));
    assert_eq!(answer["outcome"], "refused");
    assert_eq!(answer["authFailure"], Value::Null);
}

/// A refusal keeps the result it carries, which a plant's revision conflict needs to recover.
/// Catches a refusal reduced to its text.
#[test]
fn a_refusal_keeps_its_result() {
    let recovery = json!({
        "contract": "trust.plant",
        "operation": "plant.save",
        "configuration_revision": "sha256:current",
        "current_plant": null,
    });
    let runtime = refusing(json!({
        "ok": false,
        "error_code": "revision_conflict",
        "error": "saved configuration changed",
        "result": recovery,
    }));
    let answer = request(&runtime.endpoint, Some("token"), "plant.save", json!({}));
    assert_eq!(answer["code"], "revision_conflict");
    assert_eq!(answer["result"], recovery);
}

/// A refusal without `error_code` names its code by `code`, and one without text says the request
/// was rejected. Catches a refusal that loses its code or has an empty text.
#[test]
fn a_refusal_without_error_code_or_text() {
    let runtime = refusing(json!({ "ok": false, "code": "busy" }));
    let answer = request(&runtime.endpoint, None, "status", Value::Null);
    assert_eq!(answer["code"], "busy");
    assert_eq!(answer["error"], "control request rejected");
}

/// No runtime at the endpoint, and an endpoint that is no control endpoint, give the outcome
/// `transport`; an answer that is no JSON object gives `malformed`. Catches either reported as the
/// runtime's refusal.
#[test]
fn transport_and_malformed_outcomes() {
    let answer = request(&closed_endpoint(), None, "status", Value::Null);
    assert_eq!(answer["outcome"], "transport", "{answer}");
    let answer = request("http://127.0.0.1:1", None, "status", Value::Null);
    assert_eq!(answer["outcome"], "transport");
    assert_eq!(
        answer["error"],
        "invalid control endpoint 'http://127.0.0.1:1'"
    );
    let runtime = FakeRuntime::start(|_| Some("not json".to_owned()));
    let answer = request(&runtime.endpoint, None, "status", Value::Null);
    assert_eq!(answer["outcome"], "malformed", "{answer}");
}

/// A probe connects and sends nothing: a runtime that answers is reachable, a closed port is not.
/// Catches a probe that writes a request, which a busy chip would count against its one
/// connection.
#[test]
fn a_probe_connects_without_a_request() {
    let runtime = FakeRuntime::answering(json!({}));
    let reachable =
        answer(json!({ "action": "probe", "endpoint": runtime.endpoint })).expect("an answer");
    assert_eq!(reachable, json!({ "ok": true, "reachable": true }));
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(runtime.connections(), 1);
    assert!(runtime.requests().is_empty());
    let closed =
        answer(json!({ "action": "probe", "endpoint": closed_endpoint() })).expect("an answer");
    assert_eq!(closed, json!({ "ok": true, "reachable": false }));
}

/// A waiting probe tries again until the endpoint accepts: a runtime that binds its port 300 ms
/// after the probe began is reachable within a 2000 ms wait. Catches a waiting probe that gives up
/// after its first try, the false "could not connect" after a managed Start.
#[test]
fn a_waiting_probe_finds_a_runtime_that_binds_late() {
    let endpoint = closed_endpoint();
    let address = endpoint.trim_start_matches("tcp://").to_owned();
    let late = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let listener = std::net::TcpListener::bind(&address).expect("the late port");
        let _ = listener.accept();
    });
    let answer = answer(json!({ "action": "probe", "endpoint": endpoint, "waitMs": 2000 }))
        .expect("an answer");
    assert_eq!(answer["reachable"], true);
    late.join().expect("the late runtime");
}
