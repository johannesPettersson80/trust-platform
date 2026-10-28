//! Ports of the connection tests of `editors/vscode/src/test/suite/hmi-agent-pipes.test.ts` (suite
//! "HMI agent pipes follow the builder"): connect, refusal reasons, junction branches,
//! reconnection, disconnection and composition. Duplication is ported with the clipboard rules.
//!
//! Specification 43, "Repairs — 7 October 2026", item 4: the agent edits pipes with the builder's
//! own pipe editing (specification 39, §6), so it refuses what the canvas refuses, for its reason.
//! The TypeScript fixture's `instrument` category has no Rust counterpart; the flow sensor is a
//! `display` with `attaches` set, which is the only role the rules read.

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::{HmiPage, HmiPipeStyle};

use super::super::compose::compose_agent_page;
use super::super::edits::{edit_page, object_from_json, page_from_json};
use super::super::error::{AgentError, AgentResult};
use super::super::pipe_rules::add_pipe;

/// The catalogue of the TypeScript suite. The tank names a `drain` port its symbol does not draw.
fn catalogue() -> Catalogue {
    serde_json::from_value(json!({
        "format": 1,
        "components": [
            {"type": "pump", "category": "equipment", "phase": 2, "size": [128, 108], "ports": ["inlet", "outlet"]},
            {"type": "valve", "category": "equipment", "phase": 2, "size": [128, 108], "ports": ["inlet", "outlet"]},
            {"type": "junction", "category": "equipment", "phase": 2, "size": [16, 16], "ports": ["left", "right", "top", "bottom"]},
            {"type": "tank", "category": "equipment", "phase": 2, "size": [128, 172], "ports": ["inlet", "outlet", "drain"]},
            {"type": "flow-sensor", "category": "display", "phase": 2, "size": [64, 64], "attaches": true},
        ],
    }))
    .expect("the test catalogue is valid")
}

/// An object of the test page with the given type, position and size.
fn object(
    id: &str,
    kind: &str,
    at: [i64; 2],
    size: [i64; 2],
) -> trust_runtime::hmi::page_files::HmiObject {
    object_from_json(&json!({
        "id": id,
        "type": kind,
        "at": at,
        "size": size,
        "rotation": 0,
        "bind": {},
    }))
    .expect("the object is valid")
}

/// The TypeScript suite's page: two pumps, a valve, a junction and a tank, with the given pipes.
fn page(pipes: Value) -> HmiPage {
    let mut page = page_from_json(&json!({
        "title": "Pipes",
        "object": [],
        "pipe": pipes,
    }))
    .expect("the page is valid");
    page.objects = vec![
        object("p-101", "pump", [40, 40], [128, 108]),
        object("p-102", "pump", [40, 320], [128, 108]),
        object("v-101", "valve", [480, 40], [128, 108]),
        object("j-1", "junction", [400, 200], [16, 16]),
        object("tk-1", "tank", [700, 300], [128, 172]),
    ];
    page
}

/// Applies a JSON list of operations with the test catalogue.
fn run(page: &HmiPage, operations: Value) -> AgentResult<HmiPage> {
    let list = operations.as_array().cloned().unwrap_or_default();
    edit_page(page, &list, &catalogue(), None)
}

/// Asserts a refusal's stable code and that its message starts with the given text.
fn assert_refusal(error: &AgentError, code: &str, start: &str) {
    assert_eq!(error.code, code, "{error}");
    assert!(error.message.starts_with(start), "{error}");
}

/// TypeScript: "a connection the canvas accepts is joined exactly as the canvas joins it".
#[test]
fn a_connection_the_canvas_accepts_is_joined_as_the_canvas_joins_it() {
    let source = page(json!([]));
    let joined = run(
        &source,
        json!([{"op": "connect", "from": "p-101.outlet", "to": "v-101.inlet"}]),
    )
    .expect("the connection is accepted");
    let expected = add_pipe(&source, "p-101.outlet", "v-101.inlet").expect("the canvas joins it");
    assert_eq!(joined.pipes, expected.pipes);
    assert!(source.pipes.is_empty(), "the source page is unchanged");
}

/// TypeScript: "a port that already has a pipe is refused, as the canvas refuses it, with the
/// builder's reason".
#[test]
fn a_port_that_already_has_a_pipe_is_refused_with_the_builders_reason() {
    let source = page(json!([{"from": "p-101.outlet", "to": "v-101.inlet"}]));
    let error = run(
        &source,
        json!([{"op": "connect", "from": "p-101.outlet", "to": "v-101.outlet"}]),
    )
    .expect_err("the source port is taken");
    assert_refusal(
        &error,
        "CONFLICT",
        "the builder joins only free ports, and p-101.outlet already has a pipe",
    );
    let error = run(
        &source,
        json!([{"op": "connect", "from": "p-102.outlet", "to": "v-101.inlet"}]),
    )
    .expect_err("one inlet fed twice is refused");
    assert_eq!(error.code, "CONFLICT", "{error}");
    assert!(
        error.message.contains("v-101.inlet already has a pipe"),
        "{error}"
    );
    let error = run(
        &source,
        json!([{"op": "connect", "from": "p-101.outlet", "to": "v-101.inlet"}]),
    )
    .expect_err("the same pipe again is no second pipe");
    assert!(error.message.contains("already has a pipe"), "{error}");
}

/// TypeScript: "one port twice is refused with the builder's reason".
#[test]
fn one_port_twice_is_refused_with_the_builders_reason() {
    let error = run(
        &page(json!([])),
        json!([{"op": "connect", "from": "v-101.outlet", "to": "v-101.outlet"}]),
    )
    .expect_err("a port cannot join itself");
    assert_refusal(
        &error,
        "INVALID_ARGUMENT",
        "the builder joins two different ports; v-101.outlet cannot connect to itself",
    );
}

/// TypeScript: "only a port the canvas offers can be joined".
#[test]
fn only_a_port_the_canvas_offers_can_be_joined() {
    let cases = [
        ("p-101.outlet", "tk-1.drain"),
        ("p-101.outlet", "missing.inlet"),
        ("p-101.outlet", "v-101.side"),
    ];
    for (from, to) in cases {
        let error = run(
            &page(json!([])),
            json!([{"op": "connect", "from": from, "to": to}]),
        )
        .expect_err("the port is not offered");
        assert_refusal(&error, "INVALID_ARGUMENT", &format!("unknown port {to}"));
    }
}

/// TypeScript: "a branch runs through a junction, as the canvas makes one".
#[test]
fn a_branch_runs_through_a_junction_as_the_canvas_makes_one() {
    let branched = run(
        &page(json!([])),
        json!([
            {"op": "connect", "from": "p-101.outlet", "to": "j-1.left"},
            {"op": "connect", "from": "j-1.top", "to": "v-101.inlet"},
            {"op": "connect", "from": "j-1.bottom", "to": "tk-1.inlet"},
        ]),
    )
    .expect("the branch is accepted");
    let ends: Vec<(&str, &str)> = branched
        .pipes
        .iter()
        .map(|pipe| (pipe.from.as_str(), pipe.to.as_str()))
        .collect();
    assert_eq!(
        ends,
        vec![
            ("p-101.outlet", "j-1.left"),
            ("j-1.top", "v-101.inlet"),
            ("j-1.bottom", "tk-1.inlet"),
        ]
    );
}

/// TypeScript: "reconnection follows the same rule and keeps the pipe's place, style and
/// attachments".
#[test]
fn reconnection_follows_the_same_rule_and_keeps_place_style_and_attachments() {
    let mut source = page(json!([
        {"from": "p-101.outlet", "to": "v-101.inlet", "style": "signal"},
        {"from": "p-102.outlet", "to": "j-1.left"},
    ]));
    let mut sensor = object("ft-1", "flow-sensor", [240, 160], [64, 64]);
    sensor.attach = Some("pipe:0".to_owned());
    source.objects.push(sensor);
    let error = run(
        &source,
        json!([{"op": "reconnect", "from": "p-101.outlet", "to": "v-101.inlet", "newFrom": "p-101.outlet", "newTo": "j-1.left"}]),
    )
    .expect_err("j-1.left already has a pipe");
    assert_refusal(
        &error,
        "CONFLICT",
        "the builder joins only free ports, and j-1.left already has a pipe",
    );
    let moved = run(
        &source,
        json!([{"op": "reconnect", "from": "p-101.outlet", "to": "v-101.inlet", "newFrom": "p-101.outlet", "newTo": "tk-1.inlet"}]),
    )
    .expect("its own old ends are free once it moves");
    let pipe = &moved.pipes[0];
    assert_eq!(pipe.from, "p-101.outlet");
    assert_eq!(pipe.to, "tk-1.inlet");
    assert_eq!(pipe.style, HmiPipeStyle::Signal);
    let sensor = moved
        .objects
        .iter()
        .find(|candidate| candidate.id == "ft-1")
        .expect("the instrument stays on the page");
    assert_eq!(sensor.attach.as_deref(), Some("pipe:0"));
}

/// TypeScript: "a connection is named by its ends, even when one end is no longer on the page".
#[test]
fn a_connection_is_named_by_its_ends_even_when_an_end_is_no_longer_on_the_page() {
    let error = run(
        &page(json!([])),
        json!([{"op": "disconnect", "from": "p-101.outlet", "to": "v-101.inlet"}]),
    )
    .expect_err("no such connection");
    assert_refusal(
        &error,
        "INVALID_ARGUMENT",
        "unknown connection p-101.outlet -> v-101.inlet",
    );
    let broken = page(json!([{"from": "gone.outlet", "to": "v-101.inlet"}]));
    let removed = run(
        &broken,
        json!([{"op": "disconnect", "from": "gone.outlet", "to": "v-101.inlet"}]),
    )
    .expect("the named connection is removed");
    assert!(removed.pipes.is_empty());
    let repaired = run(
        &broken,
        json!([{"op": "reconnect", "from": "gone.outlet", "to": "v-101.inlet", "newFrom": "p-101.outlet", "newTo": "v-101.inlet"}]),
    )
    .expect("the repaired connection is valid");
    assert_eq!(repaired.pipes.len(), 1);
    assert_eq!(repaired.pipes[0].from, "p-101.outlet");
    assert_eq!(repaired.pipes[0].to, "v-101.inlet");
}

/// TypeScript: "composition refuses a connection the builder refuses".
#[test]
fn composition_refuses_a_connection_the_builder_refuses() {
    let base = json!({
        "title": "Transfer",
        "pattern": "process-overview",
        "size": [1280, 800],
        "items": [
            {"id": "tank", "type": "tank", "region": "process"},
            {"id": "pump-a", "type": "pump", "region": "process"},
            {"id": "pump-b", "type": "pump", "region": "process"},
        ],
    });
    let mut doubled = base.clone();
    doubled["connections"] = json!([
        {"from": "tank.outlet", "to": "pump-a.inlet"},
        {"from": "tank.outlet", "to": "pump-b.inlet"},
    ]);
    let error = compose_agent_page(&doubled, &catalogue()).expect_err("the outlet is taken");
    assert_refusal(
        &error,
        "CONFLICT",
        "the builder joins only free ports, and tank.outlet already has a pipe",
    );
    let mut single = base;
    single["connections"] = json!([{"from": "tank.outlet", "to": "pump-a.inlet"}]);
    let composed = compose_agent_page(&single, &catalogue()).expect("one connection is accepted");
    assert_eq!(composed.pipes.len(), 1);
    assert_eq!(composed.pipes[0].from, "tank.outlet");
    assert_eq!(composed.pipes[0].to, "pump-a.inlet");
}
