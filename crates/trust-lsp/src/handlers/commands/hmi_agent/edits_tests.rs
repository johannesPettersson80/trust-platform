//! Ports of `editors/vscode/src/test/suite/hmi-agent-edits.test.ts` (suite "HMI agent semantic
//! editing"): the same catalogue, base page, operations and expected values, run through the
//! operation dispatcher. Specification 43 AT-05 and the 7 October repair item 4.

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::{HmiBinding, HmiPage, HmiPipeStyle};

use super::super::error::AgentResult;
use super::{edit_page, object_from_json, page_from_json};

/// The catalogue of the TypeScript suite: one equipment type with two ports and a level property.
fn catalogue() -> Catalogue {
    serde_json::from_value(json!({
        "format": 1,
        "components": [{
            "type": "tank",
            "category": "equipment",
            "phase": 2,
            "size": [100, 100],
            "ports": ["inlet", "outlet"],
            "properties": {"level": {"accepts": ["number"], "directions": ["read"]}},
        }],
    }))
    .expect("the test catalogue is valid")
}

/// The base page of the TypeScript suite: two tanks joined by one pipe, the first labelled and
/// bound to a level.
fn base() -> HmiPage {
    page_from_json(&json!({
        "title": "Process",
        "object": [
            {
                "id": "a",
                "type": "tank",
                "at": [40, 40],
                "size": [100, 100],
                "rotation": 0,
                "bind": {"level": {"path": "Main.Level", "direction": "read"}},
                "label": "Keep me",
            },
            {"id": "b", "type": "tank", "at": [240, 80], "size": [100, 100], "rotation": 0, "bind": {}},
        ],
        "pipe": [{"from": "a.outlet", "to": "b.inlet"}],
    }))
    .expect("the base page is valid")
}

/// A tank of the test catalogue at a position, with no bindings.
fn tank(id: &str, at: [i64; 2], size: [i64; 2]) -> trust_runtime::hmi::page_files::HmiObject {
    object_from_json(&json!({
        "id": id,
        "type": "tank",
        "at": at,
        "size": size,
        "rotation": 0,
        "bind": {},
    }))
    .expect("the tank is valid")
}

/// Applies a JSON list of operations to a page with the test catalogue.
fn run(page: &HmiPage, operations: Value) -> AgentResult<HmiPage> {
    let list = operations.as_array().cloned().unwrap_or_default();
    edit_page(page, &list, &catalogue(), None)
}

/// The IDs of a page's objects, in order.
fn ids(page: &HmiPage) -> Vec<&str> {
    page.objects
        .iter()
        .map(|object| object.id.as_str())
        .collect()
}

/// The object with the given ID.
fn object<'a>(page: &'a HmiPage, id: &str) -> &'a trust_runtime::hmi::page_files::HmiObject {
    page.objects
        .iter()
        .find(|object| object.id == id)
        .expect("the page holds the object")
}

/// TypeScript: "targeted moves preserve bindings, labels, identities and other objects".
#[test]
fn targeted_moves_preserve_bindings_labels_identities_and_other_objects() {
    let original = base();
    let changed = run(
        &original,
        json!([{"op": "move", "ids": ["a"], "delta": [16, 24]}]),
    )
    .expect("the move is valid");
    assert_eq!(object(&changed, "a").at, [56, 64]);
    assert_eq!(object(&changed, "a").bind, object(&original, "a").bind);
    assert_eq!(object(&changed, "a").label.as_deref(), Some("Keep me"));
    assert_eq!(object(&changed, "b"), object(&original, "b"));
    assert_eq!(original, base());
}

/// TypeScript: "batch failure leaves the source unchanged and pinned objects cannot move".
#[test]
fn batch_failure_leaves_the_source_unchanged_and_locked_objects_cannot_move() {
    let original = base();
    let error = run(
        &original,
        json!([
            {"op": "move", "ids": ["a"], "delta": [8, 8]},
            {"op": "move", "ids": ["missing"], "delta": [8, 8]},
        ]),
    )
    .expect_err("the second operation names no object");
    assert!(
        error.message.to_lowercase().contains("unknown object"),
        "{error}"
    );
    assert_eq!(original, base());
    let mut locked = base();
    locked.objects[0].locked = true;
    let error = run(
        &locked,
        json!([{"op": "resize", "id": "a", "size": [80, 80]}]),
    )
    .expect_err("a locked object cannot be resized");
    assert!(error.message.to_lowercase().contains("locked"), "{error}");
}

/// TypeScript: "deleting connected equipment requires explicit dependent removal".
#[test]
fn deleting_connected_equipment_requires_explicit_dependent_removal() {
    let error = run(&base(), json!([{"op": "remove", "ids": ["a"]}]))
        .expect_err("the pipe depends on the object");
    assert!(
        error.message.to_lowercase().contains("dependent"),
        "{error}"
    );
    let changed = run(
        &base(),
        json!([{"op": "remove", "ids": ["a"], "removeDependents": true}]),
    )
    .expect("the dependents are named");
    assert_eq!(ids(&changed), vec!["b"]);
    assert!(changed.pipes.is_empty());
}

/// TypeScript: "port connections and binding directions respect the catalogue".
#[test]
fn port_connections_and_binding_directions_respect_the_catalogue() {
    let error = run(
        &base(),
        json!([{"op": "connect", "from": "a.fake", "to": "b.inlet"}]),
    )
    .expect_err("the port is not in the catalogue");
    assert!(error.message.to_lowercase().contains("port"), "{error}");
    let error = run(
        &base(),
        json!([{"op": "bind", "id": "a", "property": "level", "binding": {"path": "Main.Level", "direction": "write"}}]),
    )
    .expect_err("the property only reads");
    assert!(error.message.contains("direction"), "{error}");
    let changed = run(
        &base(),
        json!([{"op": "bind", "id": "a", "property": "level", "binding": {"path": "Main.Other", "direction": "read"}}]),
    )
    .expect("the binding is allowed");
    assert_eq!(object(&changed, "a").bind["level"].path, "Main.Other");
}

/// TypeScript: "align and group reuse editor geometry without changing pipes".
#[test]
fn align_and_group_reuse_editor_geometry_without_changing_pipes() {
    let changed = run(
        &base(),
        json!([
            {"op": "align", "ids": ["a", "b"], "edge": "top"},
            {"op": "group", "ids": ["a", "b"], "group": "process"},
        ]),
    )
    .expect("the batch is valid");
    assert_eq!(object(&changed, "b").at[1], 40);
    assert_eq!(object(&changed, "a").group.as_deref(), Some("process"));
    assert_eq!(changed.pipes, base().pipes);
}

/// TypeScript: "duplicate carries internal pipes and rejects duplicate ids".
#[test]
fn duplicate_carries_internal_pipes_and_rejects_duplicate_ids() {
    let changed = run(
        &base(),
        json!([{"op": "duplicate", "ids": ["a", "b"], "newIds": ["c", "d"], "delta": [0, 200]}]),
    )
    .expect("the copies are valid");
    assert_eq!(changed.objects.len(), 4);
    assert!(changed
        .pipes
        .iter()
        .any(|pipe| pipe.from == "c.outlet" && pipe.to == "d.inlet"));
    let error = run(
        &base(),
        json!([{"op": "duplicate", "ids": ["a"], "newIds": ["b"], "delta": [0, 200]}]),
    )
    .expect_err("the new ID is taken");
    assert!(error.message.to_lowercase().contains("id"), "{error}");
}

/// TypeScript: "equipment and pipe deletion keep surviving instrument attachments on their
/// original pipes".
#[test]
fn equipment_and_pipe_deletion_keep_surviving_attachments_on_their_pipes() {
    let mut original = base();
    original.objects.push(tank("c", [440, 80], [100, 100]));
    original.objects.push(
        object_from_json(&json!({
            "id": "sensor",
            "type": "tank",
            "at": [400, 240],
            "size": [80, 80],
            "rotation": 0,
            "attach": "pipe:1",
            "bind": {},
        }))
        .expect("the sensor is valid"),
    );
    original.pipes.push(
        page_from_json(
            &json!({"title": "x", "object": [], "pipe": [{"from": "b.outlet", "to": "c.inlet"}]}),
        )
        .expect("the pipe is valid")
        .pipes
        .remove(0),
    );
    let removed = run(
        &original,
        json!([{"op": "remove", "ids": ["a"], "removeDependents": true}]),
    )
    .expect("the removal is valid");
    assert_eq!(object(&removed, "sensor").attach.as_deref(), Some("pipe:0"));
    let disconnected = run(
        &original,
        json!([{"op": "disconnect", "from": "a.outlet", "to": "b.inlet"}]),
    )
    .expect("the pipe exists");
    assert_eq!(
        object(&disconnected, "sensor").attach.as_deref(),
        Some("pipe:0")
    );
    assert_eq!(
        object(&original, "sensor").attach.as_deref(),
        Some("pipe:1")
    );
}

/// TypeScript: "duplicating a connected group rebinds its pipe-attached instruments to the copied
/// pipe".
#[test]
fn duplicating_a_connected_group_rebinds_its_instruments_to_the_copied_pipe() {
    let mut original = base();
    original.objects.push(
        object_from_json(&json!({
            "id": "sensor",
            "type": "tank",
            "at": [180, 240],
            "size": [80, 80],
            "rotation": 0,
            "attach": "pipe:0",
            "bind": {},
        }))
        .expect("the sensor is valid"),
    );
    let copied = run(
        &original,
        json!([{"op": "duplicate", "ids": ["a", "b", "sensor"], "newIds": ["c", "d", "sensor-copy"], "delta": [0, 320]}]),
    )
    .expect("the copies are valid");
    assert_eq!(
        object(&copied, "sensor-copy").attach.as_deref(),
        Some("pipe:1")
    );
    assert_eq!(object(&copied, "sensor").attach.as_deref(), Some("pipe:0"));
}

/// TypeScript: "pipe reroute and reconnection preserve bindings, style and attachment identity".
#[test]
fn pipe_reroute_and_reconnection_preserve_bindings_style_and_attachment_identity() {
    let mut original = base();
    original.pipes[0].style = HmiPipeStyle::Signal;
    let flow: HmiBinding =
        serde_json::from_value(json!({"path": "Main.Flow", "direction": "read"}))
            .expect("the binding is valid");
    original.pipes[0].bind.insert("flow".to_owned(), flow);
    original.objects.push(tank("c", [440, 80], [100, 100]));
    original.objects.push(
        object_from_json(&json!({
            "id": "sensor",
            "type": "tank",
            "at": [400, 240],
            "size": [80, 80],
            "rotation": 0,
            "attach": "pipe:0",
            "bind": {},
        }))
        .expect("the sensor is valid"),
    );
    let changed = run(
        &original,
        json!([
            {"op": "reroute", "from": "a.outlet", "to": "b.inlet", "waypoints": [[200, 200], [320, 200]]},
            {"op": "reconnect", "from": "a.outlet", "to": "b.inlet", "newFrom": "a.outlet", "newTo": "c.inlet"},
        ]),
    )
    .expect("the reroute and the reconnection are valid");
    let pipe = &changed.pipes[0];
    assert_eq!(pipe.from, "a.outlet");
    assert_eq!(pipe.to, "c.inlet");
    assert_eq!(pipe.style, HmiPipeStyle::Signal);
    assert_eq!(pipe.bind, original.pipes[0].bind);
    assert_eq!(pipe.waypoints, vec![[200, 200], [320, 200]]);
    assert_eq!(object(&changed, "sensor").attach.as_deref(), Some("pipe:0"));
    let automatic = run(
        &changed,
        json!([{"op": "reroute", "from": "a.outlet", "to": "c.inlet", "waypoints": []}]),
    )
    .expect("the automatic route is valid");
    assert!(automatic.pipes[0].waypoints.is_empty());
    let error = run(
        &original,
        json!([{"op": "connect", "from": "a.outlet", "to": "a.outlet"}]),
    )
    .expect_err("a port cannot join itself");
    assert!(error.message.contains("self"), "{error}");
    let error = run(
        &original,
        json!([{"op": "reconnect", "from": "a.outlet", "to": "b.inlet", "newFrom": "a.nope", "newTo": "c.inlet"}]),
    )
    .expect_err("the port does not exist");
    assert!(error.message.to_lowercase().contains("port"), "{error}");
    assert_eq!(original.pipes[0].to, "b.inlet");
}

/// TypeScript: "equal sizing uses the reference and refuses locked or incompatible faceplate
/// geometry".
#[test]
fn equal_sizing_uses_the_reference_and_refuses_locked_or_incompatible_faceplates() {
    let mut original = base();
    original.objects[1].size = [160, 80];
    let changed = run(
        &original,
        json!([{"op": "equal-size", "ids": ["a"], "reference": "b", "axis": "both"}]),
    )
    .expect("a tank takes the reference size");
    assert_eq!(object(&changed, "a").size, [160, 80]);
    assert_eq!(object(&changed, "a").bind, object(&original, "a").bind);
    original.objects[0].kind = "faceplate".to_owned();
    let error = run(
        &original,
        json!([{"op": "equal-size", "ids": ["a"], "reference": "b", "axis": "both"}]),
    )
    .expect_err("the faceplate keeps its proportions");
    assert!(
        error.message.to_lowercase().contains("proportion"),
        "{error}"
    );
    let width = run(
        &original,
        json!([{"op": "equal-size", "ids": ["a"], "reference": "b", "axis": "width"}]),
    )
    .expect("the width is shared and the height follows the faceplate");
    assert_eq!(object(&width, "a").size, [160, 160]);
    original.objects[0].locked = true;
    let error = run(
        &original,
        json!([{"op": "equal-size", "ids": ["a"], "reference": "b", "axis": "width"}]),
    )
    .expect_err("a locked object is fixed");
    assert!(error.message.to_lowercase().contains("locked"), "{error}");
}

/// TypeScript: "regional arrangement preserves pinned and unselected objects and avoids their
/// space".
#[test]
fn regional_arrangement_preserves_pinned_and_unselected_objects_and_avoids_their_space() {
    let mut original = base();
    original.objects[0].at = [32, 88];
    original.objects[0].size = [96, 96];
    original.objects[0].locked = true;
    original.objects[1].size = [96, 96];
    original.objects.push(
        object_from_json(&json!({
            "id": "obstacle",
            "type": "tank",
            "at": [152, 88],
            "size": [96, 96],
            "rotation": 0,
            "bind": {},
        }))
        .expect("the obstacle is valid"),
    );
    let changed = edit_page(
        &original,
        &[json!({
            "op": "arrange",
            "ids": ["a", "b"],
            "region": {"x": 32, "y": 88, "width": 600, "height": 480},
            "gap": 24,
            "flow": "horizontal",
        })],
        &catalogue(),
        Some([1280, 800]),
    )
    .expect("the selection fits the region");
    assert_eq!(object(&changed, "a"), object(&original, "a"));
    assert_eq!(object(&changed, "b").at, [272, 88]);
    assert_eq!(object(&changed, "obstacle"), object(&original, "obstacle"));
    assert_eq!(changed.pipes, original.pipes);
    let error = edit_page(
        &original,
        &[json!({"op": "arrange", "ids": ["a", "b"], "region": {"x": 1200, "y": 88, "width": 600, "height": 480}})],
        &catalogue(),
        Some([1280, 800]),
    )
    .expect_err("the region leaves the page");
    assert!(error.message.to_lowercase().contains("fit"), "{error}");
}
