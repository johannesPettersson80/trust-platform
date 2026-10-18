//! The client's protocol checks against a scripted runtime: the response id, the response line
//! limit, credential failures by code, and the time budget.

use std::net::TcpListener;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::fake_runtime;
use crate::{AuthFailure, ControlClient, ControlEndpoint, ControlError, RESPONSE_LINE_LIMIT};

/// Scheduling slack on a busy test machine; far below the budgets the tests tell apart.
const SLACK: Duration = Duration::from_millis(1500);

fn connect(endpoint: &str, token: Option<&str>, timeout: Duration) -> ControlClient {
    let endpoint = ControlEndpoint::parse(endpoint).expect("a control endpoint");
    ControlClient::connect(&endpoint, token, timeout).expect("connect to the fake runtime")
}

#[test]
fn client_rejects_a_mismatched_response_id() {
    let runtime = fake_runtime::answering(vec![
        // Request 1 gets the answer of request 2.
        r#"{"id":2,"ok":true,"result":{"state":"running"}}"#.to_string(),
        // Request 2 gets the refusal of request 1: neither its answer nor its refusal.
        r#"{"id":1,"ok":false,"error":"invalid auth token","error_code":"invalid_auth_token"}"#
            .to_string(),
        // Request 3 gets an id that is not a number.
        r#"{"id":"3","ok":true,"result":{"state":"running"}}"#.to_string(),
        // Request 4 gets its own answer.
        r#"{"id":4,"ok":true,"result":{"state":"running"}}"#.to_string(),
        // Request 5 gets an answer without an id, which can only be its own.
        r#"{"ok":true,"result":{"state":"stopped"}}"#.to_string(),
    ]);
    let mut client = connect(&runtime.endpoint, None, Duration::from_secs(2));
    for _ in 0..3 {
        let answer = client.request("status", None);
        assert!(
            matches!(answer, Err(ControlError::Malformed(_))),
            "{answer:?}"
        );
    }
    assert_eq!(
        client
            .request("status", None)
            .expect("the answer with its own id"),
        json!({"state": "running"})
    );
    assert_eq!(
        client
            .request("status", None)
            .expect("the answer without an id"),
        json!({"state": "stopped"})
    );
    drop(client);
    let ids = runtime
        .requests()
        .iter()
        .map(|request| request["id"].clone())
        .collect::<Vec<_>>();
    assert_eq!(ids, [json!(1), json!(2), json!(3), json!(4), json!(5)]);
}

#[test]
fn client_maps_missing_and_rejected_tokens() {
    let runtime = fake_runtime::answering(vec![
        r#"{"id":1,"ok":false,"error":"missing auth token","error_code":"missing_auth_token"}"#
            .to_string(),
        r#"{"id":2,"ok":false,"error":"invalid auth token","error_code":"invalid_auth_token"}"#
            .to_string(),
        // The words of a credential failure without its code do not make one.
        r#"{"id":3,"ok":false,"error":"invalid auth token"}"#.to_string(),
        r#"{"id":4,"ok":false,"error":"missing auth token","error_code":"forbidden"}"#.to_string(),
    ]);
    let mut client = connect(&runtime.endpoint, Some("token"), Duration::from_secs(2));
    let mut failure = || {
        client
            .request("status", None)
            .expect_err("the runtime refuses")
            .auth_failure()
    };
    assert_eq!(failure(), Some(AuthFailure::Missing));
    assert_eq!(failure(), Some(AuthFailure::Rejected));
    assert_eq!(failure(), None);
    assert_eq!(failure(), None);
    drop(client);
    let seen = runtime.requests();
    assert_eq!(seen.len(), 4);
    assert!(
        seen.iter().all(|request| request["auth"] == "token"),
        "the token goes with every request: {seen:?}"
    );
    // Only a refusal names a credential failure.
    for error in [
        ControlError::Transport("missing_auth_token".into()),
        ControlError::Malformed("invalid_auth_token".into()),
    ] {
        assert_eq!(error.auth_failure(), None, "{error:?}");
    }
}

#[test]
fn client_bounds_the_response_size() {
    // A successful answer whose line, newline included, is `length` bytes long.
    let line = |id: u64, length: usize| {
        let head = format!(r#"{{"id":{id},"ok":true,"result":""#);
        let tail = r#""}"#;
        let padding = length - head.len() - tail.len() - 1;
        format!("{head}{}{tail}", "x".repeat(padding))
    };
    let overhead = r#"{"id":1,"ok":true,"result":""}"#.len() + 1;

    let runtime = fake_runtime::answering(vec![
        line(1, RESPONSE_LINE_LIMIT),
        line(2, RESPONSE_LINE_LIMIT + 1),
    ]);
    let mut client = connect(&runtime.endpoint, None, Duration::from_secs(5))
        .with_response_limit(RESPONSE_LINE_LIMIT);
    let at_limit = client
        .request("status", None)
        .expect("a line of exactly 1 MiB is read");
    assert_eq!(
        at_limit.as_str().map(str::len),
        Some(RESPONSE_LINE_LIMIT - overhead)
    );
    let over = client.request("status", None);
    assert!(
        matches!(&over, Err(ControlError::Malformed(reason)) if reason.contains("longer than")),
        "one byte over 1 MiB is malformed: {over:?}"
    );
    drop(client);
    assert_eq!(runtime.requests().len(), 2);

    // Without a limit the same line is read; a client that reads bulk results keeps that.
    let runtime = fake_runtime::answering(vec![line(1, RESPONSE_LINE_LIMIT + 1)]);
    let mut client = connect(&runtime.endpoint, None, Duration::from_secs(5));
    let unbounded: Value = client.request("status", None).expect("no limit by default");
    assert_eq!(
        unbounded.as_str().map(str::len),
        Some(RESPONSE_LINE_LIMIT + 1 - overhead)
    );
    drop(client);
    runtime.requests();
}

#[test]
fn client_times_out_within_its_budget() {
    let budget = Duration::from_millis(250);

    // A runtime that takes the request and never answers.
    let runtime = fake_runtime::answering(Vec::new());
    let mut client = connect(&runtime.endpoint, None, budget);
    let started = Instant::now();
    let answer = client.request("status", None);
    let waited = started.elapsed();
    assert!(
        matches!(answer, Err(ControlError::Transport(_))),
        "{answer:?}"
    );
    assert!(waited < budget + SLACK, "waited {waited:?}");
    drop(client);
    assert_eq!(runtime.requests().len(), 1);

    // A budget set after connecting replaces the one of the connection.
    let runtime = fake_runtime::answering(Vec::new());
    let mut client = connect(&runtime.endpoint, None, Duration::from_secs(10));
    client.set_timeout(budget).expect("set the budget");
    let started = Instant::now();
    let answer = client.request("status", None);
    let waited = started.elapsed();
    assert!(
        matches!(answer, Err(ControlError::Transport(_))),
        "{answer:?}"
    );
    assert!(waited < budget + SLACK, "waited {waited:?}");
    drop(client);
    runtime.requests();

    // Nothing listening: the connection fails within the budget too.
    let closed = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint =
        ControlEndpoint::parse(&format!("tcp://{}", closed.local_addr().expect("address")))
            .expect("a control endpoint");
    drop(closed);
    let started = Instant::now();
    let refused = ControlClient::connect(&endpoint, None, budget);
    assert!(
        matches!(refused, Err(ControlError::Transport(_))),
        "{refused:?}"
    );
    assert!(started.elapsed() < budget + SLACK);
}
