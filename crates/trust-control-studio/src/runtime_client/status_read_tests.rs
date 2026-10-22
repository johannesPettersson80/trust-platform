//! The status read that keeps the runtime's refusal code (`RuntimeClient::read_experiment_status`; annex E5.6 and E11.4): an
//! answer, a refusal with its code and text, and a read that gives no answer. The fake runtime answers with exactly the
//! fields that E5.6 names, for the experiment that the request names.

use serde_json::{json, Value};
use trust_control::identification::reconcile::StatusRead;
use trust_control::runtime::RuntimeConnection;

use super::fake_runtime::{answer, refusal, FakeRuntime};
use super::RuntimeClient;

/// The status record of the experiment `experiment_id` (E5.6): running, its restoration pending.
fn record(experiment_id: &Value) -> Value {
    json!({
        "experiment_id": experiment_id,
        "source": "simulation",
        "kind": "setpoint_step",
        "task": "MainTask",
        "address": "%ML0",
        "status": "running",
        "stop_reason": null,
        "restoration": "pending",
        "restoration_reason": null,
        "elapsed_ms": 1000,
        "duration_ms": 30000,
        "reviewed_scan": 41,
        "started_scan": 42,
        "last_applied": null,
        "relay_feedback": null,
        "abort_binding": null
    })
}

#[test]
fn an_answer_is_returned_as_the_runtime_gave_it() {
    let fake = FakeRuntime::start(|request| {
        Some(answer(request, record(&request["params"]["experiment_id"])))
    });
    let read =
        RuntimeClient::read_experiment_status(&fake.connection(Some("secret")), "experiment-1");
    assert_eq!(read, StatusRead::Answer(record(&json!("experiment-1"))));
    let requests = fake.requests();
    assert_eq!(
        requests.len(),
        1,
        "a read sends the status request and nothing else"
    );
    assert_eq!(requests[0]["type"], "control.experiment.status");
    assert_eq!(
        requests[0]["params"],
        json!({ "experiment_id": "experiment-1" })
    );
    assert_eq!(
        requests[0]["auth"], "secret",
        "the control token goes with the request"
    );
}

#[test]
fn a_refusal_keeps_the_runtimes_code_and_text() {
    for (code, text) in [
        (
            "experiment_not_found",
            "No experiment holds the identifier experiment-1.",
        ),
        ("invalid_auth_token", "The control token is wrong."),
    ] {
        let fake = FakeRuntime::start(move |request| Some(refusal(request, code, text)));
        let read = RuntimeClient::read_experiment_status(&fake.connection(None), "experiment-1");
        assert_eq!(
            read,
            StatusRead::Refused {
                code: Some(code.to_string()),
                text: text.to_string()
            }
        );
    }
}

#[test]
fn a_runtime_that_cannot_be_reached_or_does_not_answer_properly_is_unreadable() {
    let not_an_endpoint = RuntimeConnection {
        endpoint: "http://localhost:9".to_string(),
        token: None,
        runtime_id: None,
    };
    assert_eq!(
        RuntimeClient::read_experiment_status(&not_an_endpoint, "experiment-1"),
        StatusRead::Unreadable("http://localhost:9 is not a runtime endpoint.".to_string())
    );

    // A runtime that closes the connection without an answer: a lost reply (E11.4).
    let silent = FakeRuntime::start(|_| None);
    assert_eq!(
        RuntimeClient::read_experiment_status(&silent.connection(None), "experiment-1"),
        StatusRead::Unreadable("the runtime closed the connection".to_string())
    );

    // An answer that is not a control response (E0.1).
    let strange = FakeRuntime::start(|_| Some(json!({ "status": "running" })));
    let read = RuntimeClient::read_experiment_status(&strange.connection(None), "experiment-1");
    let StatusRead::Unreadable(text) = read else {
        panic!("an answer outside the envelope is unreadable");
    };
    assert!(
        text.starts_with("the runtime's answer is not a control response"),
        "{text}"
    );
}
