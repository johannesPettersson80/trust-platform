//! Tool calls (specification 43, §3.1): the checks, the workspace, the request ids, the eight calls
//! in progress, cancellation, the call's time and the failure answer.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::bridge::{CALL, CANCEL, TOOLS};
use super::super::Limits;
use super::{
    call, cancel, cancelled_result, ping, refusal, timed, unavailable, Calls, FakeBridge, Session,
    CAPABILITY, WORKSPACE,
};

/// The arguments of the examples' calls.
fn arguments() -> Value {
    json!({"contractVersion": 1, "input": {"draftId": "d1"}})
}

/// Catches: a tool call reaching the loopback with another name, without the connection's
/// workspace or with other arguments, or the tool's result changing on the way back.
#[test]
fn a_call_runs_the_named_tool_for_the_connections_workspace() {
    let result = json!({
        "content": [
            {"type": "text", "text": "{\"ok\":true}"},
            {"type": "image", "mimeType": "image/png", "data": "iVBORw0KGgo="}
        ],
        "structuredContent": {"ok": true},
        "isError": false
    });
    let bridge = FakeBridge::start(Calls::Answer(result.clone()));
    let session = Session::start(&bridge, Limits::PRODUCT);
    assert_eq!(
        session.ask(&call(json!(4), "hmi_render_draft", arguments())),
        json!({"jsonrpc": "2.0", "id": 4, "result": result})
    );
    let named = json!({"contractVersion": 1, "input": {}, "workspaceId": WORKSPACE});
    assert_eq!(
        session.ask(&call(json!("five"), "hmi_get_context", named.clone()))["result"],
        result
    );
    let calls = bridge.received_on(CALL);
    assert_eq!(calls.len(), 2);
    let expected = [
        (
            "hmi_render_draft",
            json!({"contractVersion": 1, "input": {"draftId": "d1"}, "workspaceId": WORKSPACE}),
        ),
        ("hmi_get_context", named),
    ];
    for (received, (name, arguments)) in calls.iter().zip(expected) {
        assert_eq!(received.body["name"], name);
        assert_eq!(received.body["arguments"], arguments);
        assert!(received.body["id"].is_string(), "the loopback's call id");
    }
    assert_ne!(
        calls[0].body["id"], calls[1].body["id"],
        "each call has its own loopback id"
    );
    session.close();
}

/// Catches: an arbitrary name reaching the loopback (an open command executor), or a call whose
/// arguments are not an object.
#[test]
fn a_call_names_a_listed_tool_with_an_argument_object() {
    let bridge = FakeBridge::start(Calls::Answer(json!({"content": []})));
    let session = Session::start(&bridge, Limits::PRODUCT);
    let unknown = "Unknown authoring tool.";
    let not_object = "Tool arguments must be an object.";
    for (params, reason) in [
        (
            json!({"name": "vscode.executeCommand", "arguments": {}}),
            unknown,
        ),
        (json!({"name": "hmi_unknown", "arguments": {}}), unknown),
        (json!({"name": 7, "arguments": {}}), unknown),
        (json!({"arguments": {}}), unknown),
        (Value::Null, unknown),
        (
            json!({"name": "hmi_get_context", "arguments": []}),
            not_object,
        ),
        (
            json!({"name": "hmi_get_context", "arguments": "{}"}),
            not_object,
        ),
        (
            json!({"name": "hmi_get_context", "arguments": null}),
            not_object,
        ),
        (json!({"name": "hmi_get_context"}), not_object),
    ] {
        let request = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": params});
        assert_eq!(
            session.ask(&request),
            refusal(json!(3), -32602, reason),
            "{params}"
        );
    }
    assert!(bridge.received_on(CALL).is_empty());
    session.close();
}

/// Catches: a call for another workspace reaching the loopback.
#[test]
fn a_call_for_another_workspace_is_refused() {
    let bridge = FakeBridge::start(Calls::Answer(json!({"content": []})));
    let session = Session::start(&bridge, Limits::PRODUCT);
    for workspace in [json!("project-b"), Value::Null, json!(1), json!("")] {
        let arguments = json!({"contractVersion": 1, "input": {}, "workspaceId": workspace});
        assert_eq!(
            session.ask(&call(json!(6), "hmi_get_context", arguments)),
            refusal(json!(6), -32602, "The tool belongs to another workspace."),
            "{workspace}"
        );
    }
    assert!(bridge.received_on(CALL).is_empty());
    session.close();
}

/// Catches: two calls in progress with one request id, whose cancellation would be ambiguous, or a
/// string id being taken for the number it spells.
#[test]
fn a_request_id_in_progress_is_refused() {
    let bridge = FakeBridge::start(Calls::UntilCancelled);
    let session = Session::start(&bridge, Limits::PRODUCT);
    session.send(&call(json!(7), "hmi_get_context", arguments()));
    bridge.wait_for(CALL, 1);
    assert_eq!(
        session.ask(&call(json!(7), "hmi_get_context", arguments())),
        refusal(
            json!(7),
            -32600,
            "Duplicate request id or too many active requests."
        )
    );
    session.send(&call(json!("7"), "hmi_get_context", arguments()));
    assert_eq!(bridge.wait_for(CALL, 2).len(), 2, "\"7\" is another id");
    session.send(&cancel(json!(7)));
    session.send(&cancel(json!("7")));
    let answered: HashSet<String> = (0..2).map(|_| session.next()["id"].to_string()).collect();
    assert_eq!(
        answered,
        HashSet::from(["7".to_owned(), "\"7\"".to_owned()])
    );
    session.send(&call(json!(7), "hmi_render_draft", arguments()));
    assert_eq!(
        bridge.wait_for(CALL, 3).len(),
        3,
        "an answered id is free again"
    );
    session.send(&cancel(json!(7)));
    assert_eq!(session.next()["id"], json!(7));
    session.close();
}

/// Catches: more than eight calls running at once, or the server no longer answering while they
/// run.
#[test]
fn the_ninth_call_in_progress_is_refused_while_ping_is_answered() {
    let bridge = FakeBridge::start(Calls::UntilCancelled);
    let session = Session::start(&bridge, Limits::PRODUCT);
    for id in 1..=8 {
        session.send(&call(json!(id), "hmi_get_context", arguments()));
    }
    bridge.wait_for(CALL, 8);
    assert_eq!(
        session.ask(&call(json!(9), "hmi_get_context", arguments())),
        refusal(
            json!(9),
            -32600,
            "Duplicate request id or too many active requests."
        )
    );
    assert_eq!(
        session.ask(&ping(json!(10))),
        json!({"jsonrpc": "2.0", "id": 10, "result": {}})
    );
    for id in 1..=8 {
        session.send(&cancel(json!(id)));
    }
    let mut answered: Vec<i64> = (0..8)
        .map(|_| {
            let answer = session.next();
            assert_eq!(answer["result"], cancelled_result());
            answer["id"].as_i64().expect("a numeric id")
        })
        .collect();
    answered.sort_unstable();
    assert_eq!(answered, (1..=8).collect::<Vec<i64>>());
    assert_eq!(bridge.received_on(CALL).len(), 8, "the ninth never ran");
    session.close();
}

/// Catches: cancellation not reaching the loopback, reaching it for another call, or the cancelled
/// call's answer being lost.
#[test]
fn cancellation_reaches_the_loopback_for_that_call_only() {
    let bridge = FakeBridge::start(Calls::UntilCancelled);
    let session = Session::start(&bridge, Limits::PRODUCT);
    session.send(&call(json!(7), "hmi_get_context", arguments()));
    session.send(&call(json!(8), "hmi_render_draft", arguments()));
    let calls = bridge.wait_for(CALL, 2);
    let loopback_id = |name: &str| {
        calls
            .iter()
            .find(|received| received.body["name"] == name)
            .map(|received| received.body["id"].clone())
            .expect("the call reached the loopback")
    };
    session.send(&cancel(json!(99)));
    session.send(&cancel(json!("7")));
    session.send(&json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {}}));
    assert!(session.silent_for(Duration::from_millis(300)));
    assert!(
        bridge.received_on(CANCEL).is_empty(),
        "no call has those ids"
    );
    session.send(&cancel(json!(7)));
    assert_eq!(
        session.next(),
        json!({"jsonrpc": "2.0", "id": 7, "result": cancelled_result()})
    );
    let cancels = bridge.received_on(CANCEL);
    assert_eq!(cancels.len(), 1);
    assert_eq!(
        cancels[0].body,
        json!({"id": loopback_id("hmi_get_context")})
    );
    assert!(
        session.silent_for(Duration::from_millis(300)),
        "call 8 is still running"
    );
    session.send(&cancel(json!(8)));
    assert_eq!(session.next()["id"], json!(8));
    assert_eq!(
        bridge.received_on(CANCEL)[1].body,
        json!({"id": loopback_id("hmi_render_draft")})
    );
    session.close();
}

/// Catches: a call that runs past its time without being cancelled at the loopback, or one that is
/// cancelled before its time.
#[test]
fn a_call_past_its_time_is_cancelled_at_the_loopback() {
    let bridge = FakeBridge::start(Calls::UntilCancelled);
    let session = Session::start(
        &bridge,
        timed(Duration::from_millis(400), Duration::from_secs(10)),
    );
    let sent = Instant::now();
    assert_eq!(
        session.ask(&call(json!(1), "hmi_get_context", arguments())),
        json!({"jsonrpc": "2.0", "id": 1, "result": cancelled_result()}),
        "the tool answers its cancellation"
    );
    assert!(
        sent.elapsed() >= Duration::from_millis(400),
        "not before its time"
    );
    let calls = bridge.received_on(CALL);
    assert_eq!(
        bridge.received_on(CANCEL)[0].body,
        json!({"id": calls[0].body["id"]})
    );
    session.close();
}

/// Catches: a call whose loopback never answers holding the agent's request forever, or being
/// answered before the exchange's time.
#[test]
fn a_call_without_an_answer_in_time_fails() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(
        &bridge,
        timed(Duration::from_millis(200), Duration::from_millis(600)),
    );
    let sent = Instant::now();
    assert_eq!(
        session.ask(&call(json!("slow"), "hmi_get_context", arguments())),
        unavailable(json!("slow"))
    );
    let waited = sent.elapsed();
    assert!(waited >= Duration::from_millis(600), "{waited:?}");
    assert!(waited < Duration::from_secs(5), "{waited:?}");
    assert_eq!(
        bridge.received_on(CANCEL).len(),
        1,
        "cancelled at its time first"
    );
    session.close();
}

/// Catches: a refused or failed loopback exchange being reported as a result, or not at all.
#[test]
fn a_failed_exchange_answers_the_connection_failure() {
    for status in [500, 401, 404] {
        let bridge = FakeBridge::start(Calls::Status(status));
        let session = Session::start(&bridge, Limits::PRODUCT);
        assert_eq!(
            session.ask(&call(json!(2), "hmi_get_context", arguments())),
            unavailable(json!(2)),
            "status {status}"
        );
        session.close();
    }
    let bridge = FakeBridge::start(Calls::Answer(json!({"content": []})));
    let session = Session::start(&bridge, Limits::PRODUCT);
    drop(bridge);
    assert_eq!(
        session.ask(&call(json!(3), "hmi_get_context", arguments())),
        unavailable(json!(3)),
        "the window closed"
    );
    session.close();
}

/// Catches: the end of standard input dropping the answers of calls still in progress.
#[test]
fn calls_in_progress_answer_before_the_server_ends() {
    let bridge = FakeBridge::start(Calls::Late(
        Duration::from_millis(300),
        json!({"content": []}),
    ));
    let session = Session::start(&bridge, Limits::PRODUCT);
    session.send(&call(json!(1), "hmi_get_context", arguments()));
    let lines = session.close();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        serde_json::from_str::<Value>(&lines[0]).expect("JSON"),
        json!({"jsonrpc": "2.0", "id": 1, "result": {"content": []}})
    );
}

/// Catches: an exchange without the capability, or on another route than the loopback's three.
#[test]
fn every_exchange_carries_the_capability_on_a_loopback_route() {
    let bridge = FakeBridge::start(Calls::UntilCancelled);
    let session = Session::start(&bridge, Limits::PRODUCT);
    session.send(&call(json!(1), "hmi_get_context", arguments()));
    bridge.wait_for(CALL, 1);
    session.send(&cancel(json!(1)));
    session.next();
    session.close();
    let received = bridge.received();
    let routes: Vec<&str> = received
        .iter()
        .map(|request| request.route.as_str())
        .collect();
    assert_eq!(routes, [TOOLS, CALL, CANCEL]);
    for request in &received {
        assert_eq!(request.authorization, format!("Bearer {CAPABILITY}"));
    }
}
