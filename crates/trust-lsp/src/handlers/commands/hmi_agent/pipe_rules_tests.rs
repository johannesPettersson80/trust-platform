//! Tests of the builder's pipe rules (specification 39, §6). Ported from `addPipe` and `deletePipes`
//! assertions of `editors/vscode/src/test/suite/hmi-builder-pipes.test.ts`, `hmi-agent-pipes.test.ts` and
//! `hmi-agent-edits.test.ts`. The refusal sentences of connect, reconnect and disconnect belong to the
//! agent's pipe operations and are not ported here; neither are composition's connections.

use std::collections::BTreeMap;

use trust_runtime::hmi::page_files::{HmiObject, HmiPage, HmiPipe, HmiPipeStyle};

use super::{add_pipe, delete_pipes, pipe_index, pipe_name};

fn object(id: &str, kind: &str, at: [i64; 2]) -> HmiObject {
    serde_json::from_value(
        serde_json::json!({ "id": id, "type": kind, "at": at, "size": [128, 108] }),
    )
    .expect("a page object with its required fields")
}

fn attached(mut object: HmiObject, attach: &str) -> HmiObject {
    object.attach = Some(attach.to_string());
    object
}

fn pipe(from: &str, to: &str) -> HmiPipe {
    HmiPipe {
        from: from.to_string(),
        to: to.to_string(),
        style: HmiPipeStyle::Process,
        waypoints: Vec::new(),
        bind: BTreeMap::new(),
    }
}

fn page(objects: Vec<HmiObject>, pipes: Vec<HmiPipe>) -> HmiPage {
    HmiPage {
        regions: Vec::new(),
        title: "Pipes".to_string(),
        order: None,
        icon: None,
        group: None,
        hidden: false,
        incomplete: false,
        pid: None,
        objects,
        pipes,
    }
}

fn pump_and_valve() -> Vec<HmiObject> {
    vec![
        object("p-101", "pump", [40, 40]),
        object("v-101", "valve", [480, 40]),
    ]
}

// hmi-builder-pipes.test.ts: "a pipe joins two free ports" and hmi-agent-pipes.test.ts: "a connection the
// canvas accepts is joined exactly as the canvas joins it" (specification 39, §6)
#[test]
fn a_pipe_joins_two_free_ports_with_the_defaults_of_a_new_pipe() {
    let source = page(pump_and_valve(), Vec::new());
    let joined =
        add_pipe(&source, "p-101.outlet", "v-101.inlet").expect("two free ports are joined");
    assert_eq!(joined.pipes, vec![pipe("p-101.outlet", "v-101.inlet")]);
    assert_eq!(joined.pipes[0].style, HmiPipeStyle::Process);
    assert!(
        joined.pipes[0].waypoints.is_empty(),
        "a new pipe has no bends"
    );
    assert!(
        joined.pipes[0].bind.is_empty(),
        "a new pipe has no bindings"
    );
    assert!(source.pipes.is_empty(), "the source page is unchanged");
}

// hmi-builder-pipes.test.ts: "a port that has a pipe ... is refused"; hmi-agent-pipes.test.ts: the outlet
// that already has a pipe (specification 39, §6)
#[test]
fn a_port_that_has_a_pipe_is_refused() {
    let source = page(pump_and_valve(), vec![pipe("p-101.outlet", "v-101.inlet")]);
    assert_eq!(
        add_pipe(&source, "p-101.outlet", "v-101.outlet"),
        None,
        "the outlet already has a pipe"
    );
    assert_eq!(
        add_pipe(&source, "p-101.outlet", "v-101.inlet"),
        None,
        "the same pipe again is no second pipe"
    );
}

// hmi-agent-pipes.test.ts: "one inlet fed twice is refused too" (specification 39, §6)
#[test]
fn a_port_fed_by_a_pipe_as_its_end_is_refused_too() {
    let source = page(
        vec![
            object("p-101", "pump", [40, 40]),
            object("p-102", "pump", [40, 320]),
            object("v-101", "valve", [480, 40]),
        ],
        vec![pipe("p-101.outlet", "v-101.inlet")],
    );
    assert_eq!(add_pipe(&source, "p-102.outlet", "v-101.inlet"), None);
}

// hmi-builder-pipes.test.ts: "a port to itself" and hmi-agent-pipes.test.ts: "one port twice is refused"
// (specification 39, §6)
#[test]
fn one_port_twice_is_refused() {
    let source = page(pump_and_valve(), Vec::new());
    assert_eq!(add_pipe(&source, "v-101.outlet", "v-101.outlet"), None);
}

// hmi-agent-pipes.test.ts: "a branch runs through a junction, as the canvas makes one" (specification 39,
// §6 item 2; the junction's ports are offered by the catalogue, which the agent's connect checks)
#[test]
fn a_branch_runs_through_a_junction() {
    let mut branched = page(pump_and_valve(), Vec::new());
    branched.objects.push(object("j-1", "junction", [400, 200]));
    for (from, to) in [
        ("p-101.outlet", "j-1.left"),
        ("j-1.top", "v-101.inlet"),
        ("j-1.bottom", "tk-1.inlet"),
    ] {
        branched = add_pipe(&branched, from, to).expect("each port of the branch is free");
    }
    assert_eq!(
        branched.pipes,
        vec![
            pipe("p-101.outlet", "j-1.left"),
            pipe("j-1.top", "v-101.inlet"),
            pipe("j-1.bottom", "tk-1.inlet"),
        ]
    );
}

// hmi-builder-pipes.test.ts: "deleting a pipe leaves the objects", deletePipes part (specification 39, §6)
#[test]
fn deleting_a_pipe_leaves_the_objects() {
    let joined = page(
        pump_and_valve(),
        vec![
            pipe("p-101.outlet", "v-101.inlet"),
            pipe("v-101.outlet", "p-101.inlet"),
        ],
    );
    let fewer = delete_pipes(&joined, &["pipe:0".to_string()]);
    assert_eq!(fewer.pipes, vec![pipe("v-101.outlet", "p-101.inlet")]);
    assert_eq!(fewer.objects.len(), 2);
}

// hmi-agent-edits.test.ts: "equipment and pipe deletion keep surviving instrument attachments on their
// original pipes", disconnect part (specification 39, §6 item 5)
#[test]
fn disconnecting_a_pipe_keeps_the_attachments_of_the_later_pipes() {
    let before = page(
        vec![
            object("a", "tank", [40, 40]),
            object("b", "tank", [240, 80]),
            object("c", "tank", [440, 80]),
            attached(object("sensor", "tank", [400, 240]), "pipe:1"),
        ],
        vec![pipe("a.outlet", "b.inlet"), pipe("b.outlet", "c.inlet")],
    );
    let after = delete_pipes(&before, &["pipe:0".to_string()]);
    let sensor = after.objects.iter().find(|object| object.id == "sensor");
    assert_eq!(
        sensor.and_then(|object| object.attach.as_deref()),
        Some("pipe:0")
    );
    assert_eq!(
        before.objects[3].attach.as_deref(),
        Some("pipe:1"),
        "the source page is unchanged"
    );
}

// hmi-agent-pipes.test.ts: "a connection is named by its ends, even when one end is no longer on the page",
// the removal of the connection (specification 39, §6)
#[test]
fn a_connection_whose_end_is_gone_is_removed_by_its_place() {
    let broken = page(pump_and_valve(), vec![pipe("gone.outlet", "v-101.inlet")]);
    assert_eq!(
        delete_pipes(&broken, &["pipe:0".to_string()]).pipes,
        Vec::new()
    );
}

// Boundary: a name that is not a pipe name is ignored, and a page with no pipe to delete is unchanged.
#[test]
fn names_that_are_not_pipe_names_change_nothing() {
    let source = page(pump_and_valve(), vec![pipe("p-101.outlet", "v-101.inlet")]);
    assert_eq!(
        delete_pipes(&source, &["p-101".to_string(), "pipe:x".to_string()]),
        source
    );
}

// Boundary: a well-formed name beyond the page's pipes removes no pipe, and an attachment to a place that
// stays keeps its place (`deletePipes` takes the name as a place, pipes.ts line 24).
#[test]
fn a_pipe_name_beyond_the_pipes_removes_no_pipe() {
    let source = page(
        vec![attached(object("ft-1", "flow-sensor", [0, 0]), "pipe:1")],
        vec![
            pipe("p-101.outlet", "v-101.inlet"),
            pipe("v-101.outlet", "p-101.inlet"),
        ],
    );
    let after = delete_pipes(&source, &["pipe:9".to_string()]);
    assert_eq!(after.pipes, source.pipes);
    assert_eq!(after.objects[0].attach.as_deref(), Some("pipe:1"));
}

// Boundary: a name given twice deletes its pipe once (`pipes.ts` line 24 collects the places in a set).
#[test]
fn a_repeated_pipe_name_deletes_its_pipe_once() {
    let source = page(
        pump_and_valve(),
        vec![
            pipe("p-101.outlet", "v-101.inlet"),
            pipe("v-101.outlet", "p-101.inlet"),
        ],
    );
    let after = delete_pipes(&source, &["pipe:0".to_string(), "pipe:0".to_string()]);
    assert_eq!(after.pipes, vec![pipe("v-101.outlet", "p-101.inlet")]);
}

// Closed form of the renumbering (attachments.ts lines 33-38): with pipes 0 and 2 of five deleted, the
// attachments to pipes 1, 2, 3 and 4 become pipe 0, nothing, pipe 1 and pipe 2.
#[test]
fn deleting_pipes_renumbers_the_attachments_of_the_pipes_that_stay() {
    let source = page(
        vec![
            attached(object("s1", "flow-sensor", [0, 0]), "pipe:1"),
            attached(object("s2", "flow-sensor", [0, 0]), "pipe:2"),
            attached(object("s3", "flow-sensor", [0, 0]), "pipe:3"),
            attached(object("s4", "flow-sensor", [0, 0]), "pipe:4"),
        ],
        vec![
            pipe("a.outlet", "b.inlet"),
            pipe("b.outlet", "c.inlet"),
            pipe("c.outlet", "d.inlet"),
            pipe("d.outlet", "e.inlet"),
            pipe("e.outlet", "f.inlet"),
        ],
    );
    let after = delete_pipes(&source, &["pipe:0".to_string(), "pipe:2".to_string()]);
    assert_eq!(
        after.pipes,
        vec![
            pipe("b.outlet", "c.inlet"),
            pipe("d.outlet", "e.inlet"),
            pipe("e.outlet", "f.inlet")
        ]
    );
    let attaches: Vec<Option<&str>> = after
        .objects
        .iter()
        .map(|object| object.attach.as_deref())
        .collect();
    assert_eq!(
        attaches,
        [Some("pipe:0"), None, Some("pipe:1"), Some("pipe:2")]
    );
}

// Closed form and boundary of `pipeIndex` (pipes.ts line 30, the expression `^pipe:(\d+)$`).
#[test]
fn a_pipe_name_is_pipe_colon_and_digits_only() {
    assert_eq!(pipe_index("pipe:0"), Some(0));
    assert_eq!(pipe_index("pipe:12"), Some(12));
    assert_eq!(pipe_index("pipe:007"), Some(7));
    for text in [
        "pipe:",
        "pipe:+1",
        "pipe:-1",
        "pipe:1 ",
        "pipe: 1",
        "Pipe:1",
        "pipe:1x",
        "pipe:1.5",
        "xpipe:1",
        "p-101",
        "pipe:99999999999999999999999999",
    ] {
        assert_eq!(pipe_index(text), None, "{text} is not a pipe name");
    }
}

#[test]
fn a_pipe_is_named_by_its_place() {
    assert_eq!(pipe_name(3), "pipe:3");
}
