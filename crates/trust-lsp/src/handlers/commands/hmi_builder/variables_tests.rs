//! The variable picker's rules (specification 39, §7.1, §7.4, §7.6, §8.7): the extension's former
//! `hmi-builder-variables.test.ts` picker cases, moved here with the rules, and the binding and
//! drop answers the inspector and the canvas now take from the language server.

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::Catalogue;

use super::answer;

fn value(path: &str, type_name: &str, kinds: &[&str]) -> Value {
    json!({
        "name": path.rsplit('.').next().unwrap(),
        "path": path,
        "kind": "value",
        "type": type_name,
        "kinds": kinds,
        "writable": true,
    })
}

fn speed() -> Value {
    value("Main.Speed", "REAL", &["number", "value"])
}

fn running() -> Value {
    let mut node = value("Main.P101.Running", "BOOL", &["boolean", "value"]);
    node["writable"] = json!(false);
    node["writeBlock"] = json!("is an output of Pump");
    node
}

fn limit() -> Value {
    let mut node = value("Main.Limit", "REAL", &["number", "value"]);
    node["writable"] = json!(false);
    node["writeBlock"] = json!("is a constant");
    node
}

fn run() -> Value {
    value("Main.Run", "BOOL", &["boolean", "value"])
}

fn p101() -> Value {
    json!({ "name": "P101", "path": "Main.P101", "kind": "function-block", "type": "Pump",
        "kinds": [], "writable": true, "children": [running()] })
}

/// The program `Main`: `P101 : Pump` with its output `Running`, `Speed`, the constant `Limit`
/// and the writable boolean `Run`.
fn tree() -> Value {
    json!([{ "name": "Main", "path": "Main", "kind": "program", "type": "Main", "kinds": [],
        "writable": true, "children": [p101(), speed(), limit(), run()] }])
}

/// A shipped component's definition, as the editor's catalogue gives it.
fn component(kind: &str) -> Value {
    serde_json::to_value(
        Catalogue::shipped()
            .component(kind)
            .unwrap_or_else(|| panic!("the shipped {kind}")),
    )
    .unwrap()
}

fn offered(request: Value) -> Value {
    let mut request = request;
    request["action"] = json!("offered");
    let reply = answer(request).expect("an answer");
    assert_eq!(reply["ok"], json!(true), "{reply}");
    reply["rows"].clone()
}

/// The rows as `path` (or `path*` where the binding may have it), depth first.
fn listed(rows: &Value) -> Vec<String> {
    rows.as_array()
        .unwrap()
        .iter()
        .flat_map(|row| {
            let path = row["node"]["path"].as_str().unwrap();
            let mark = if row["offered"] == json!(true) {
                "*"
            } else {
                ""
            };
            std::iter::once(format!("{path}{mark}")).chain(listed(&row["children"]))
        })
        .collect()
}

fn bind(request: Value) -> Value {
    let mut request = request;
    request["action"] = json!("bind");
    answer(request).expect("an answer")
}

fn switch() -> Value {
    json!({ "id": "pump-switch", "type": "switch", "at": [40, 400], "size": [88, 48],
        "rotation": 0, "bind": {} })
}

#[test]
fn a_binding_is_offered_only_variables_of_a_kind_it_accepts() {
    // Wrong behaviour caught: a status's state (a boolean, an enumeration or a number) offered
    // a text, or a gauge's number offered a boolean.
    let rows = offered(json!({ "variables": [
            value("Main.Name", "STRING", &["text", "value"]), run(), speed()],
        "component": component("status"), "type": "status", "property": "state" }));
    assert_eq!(listed(&rows), ["Main.Run*", "Main.Speed*"]);
    let rows = offered(
        json!({ "variables": [run(), speed()], "component": component("gauge"),
        "type": "gauge", "property": "value" }),
    );
    assert_eq!(listed(&rows), ["Main.Speed*"]);
}

#[test]
fn a_binding_that_writes_is_offered_only_variables_that_can_be_written() {
    // Wrong behaviour caught: a numeric input, which writes, offered the constant `Limit`; or a
    // setpoint's low limit, which only reads, refusing it.
    let rows = offered(json!({ "variables": [speed(), limit()],
        "component": component("numeric-input"), "type": "numeric-input", "property": "value" }));
    assert_eq!(listed(&rows), ["Main.Speed*"]);
    let rows = offered(
        json!({ "variables": [speed(), limit()], "component": component("setpoint"),
        "type": "setpoint", "property": "low-limit" }),
    );
    assert_eq!(listed(&rows), ["Main.Speed*", "Main.Limit*"]);
    // A binding that already only reads keeps its direction, so it is offered the constant.
    let rows = offered(
        json!({ "variables": [limit()], "component": component("numeric-input"),
        "type": "numeric-input", "property": "value", "direction": "read" }),
    );
    assert_eq!(listed(&rows), ["Main.Limit*"]);
}

#[test]
fn the_picker_keeps_the_variables_it_offers_and_the_programs_and_blocks_that_hold_them() {
    // Wrong behaviour caught: the picker listing every variable, losing the block that holds the
    // one offered, or letting the program and the block be picked themselves.
    let rows = offered(
        json!({ "variables": tree(), "component": component("status"),
        "type": "status", "property": "visible" }),
    );
    assert_eq!(
        listed(&rows),
        ["Main", "Main.P101", "Main.P101.Running*", "Main.Run*"]
    );
    let main = &rows[0];
    assert_eq!(main["node"]["kind"], json!("program"));
    assert_eq!(
        main["node"].get("children"),
        None,
        "a row carries its node alone"
    );
    assert_eq!(
        main["children"][0]["children"][0]["node"]["writeBlock"],
        json!("is an output of Pump")
    );
}

#[test]
fn an_array_lists_its_element_under_it() {
    // Wrong behaviour caught: an array's element, which stands for all its elements, missing
    // from the picker, so no element of an array could be bound.
    let mut element = value("Main.Levels[1]", "REAL", &["number", "value"]);
    element["name"] = json!("[1]");
    let levels = json!({ "name": "Levels", "path": "Main.Levels", "kind": "array",
        "type": "ARRAY[1..4] OF REAL", "kinds": ["array"], "writable": true,
        "array": { "dimensions": [[1, 4]], "element": element } });
    let rows = offered(
        json!({ "variables": [levels.clone()], "component": component("gauge"),
        "type": "gauge", "property": "value" }),
    );
    assert_eq!(listed(&rows), ["Main.Levels", "Main.Levels[1]*"]);
    let rows = offered(
        json!({ "variables": [levels], "component": component("table"),
        "type": "table", "property": "value" }),
    );
    assert_eq!(
        listed(&rows),
        ["Main.Levels*"],
        "a table takes the whole array"
    );
}

#[test]
fn a_faceplates_fields_come_first_as_dollar_names_with_the_kinds_they_take() {
    // §8.7: a field binds as `$Name`; one naming no kind takes any single value, and a single
    // value is also a "value". Wrong behaviour caught: the fields missing from a panel's picker,
    // a field of numbers offered to a boolean, or a field naming no kind refused a boolean.
    let fields = json!({ "Spare": { "accepts": ["number"] }, "Running": { "accepts": ["boolean"] },
        "Any": { "accepts": [] } });
    let rows = offered(json!({ "variables": [speed()], "fields": fields,
        "component": component("value"), "type": "value", "property": "value" }));
    assert_eq!(
        listed(&rows),
        ["$Any*", "$Running*", "$Spare*", "Main.Speed*"]
    );
    assert_eq!(rows[0]["node"]["type"], json!("any value"));
    assert_eq!(rows[2]["node"]["type"], json!("number"));
    let rows = offered(json!({ "variables": [], "fields": fields,
        "component": component("switch"), "type": "switch", "property": "value" }));
    assert_eq!(listed(&rows), ["$Any*", "$Running*"]);
}

#[test]
fn a_pipes_flow_is_offered_numbers_and_booleans() {
    // §6 item 6: a pipe shows flow from a number or a boolean. Wrong behaviour caught: a text
    // offered as a flow, or a boolean flow signal refused.
    let rows = offered(json!({ "variables": [
            value("Main.Name", "STRING", &["text", "value"]), run(), speed()],
        "type": "pipe", "property": "flow" }));
    assert_eq!(listed(&rows), ["Main.Run*", "Main.Speed*"]);
}

#[test]
fn a_property_the_component_does_not_have_is_offered_nothing() {
    // Wrong behaviour caught: the picker of an unknown binding offering every variable.
    let rows = offered(
        json!({ "variables": tree(), "component": component("value"),
        "type": "value", "property": "colour" }),
    );
    assert_eq!(listed(&rows), Vec::<String>::new());
}

#[test]
fn a_bindings_direction_is_its_own_else_the_first_its_property_allows() {
    // §7.4. Wrong behaviour caught: a switch's new binding written as `read`, so it could never
    // switch; or a binding the user set to `write` turned back into `read-write`.
    let made = bind(
        json!({ "component": component("switch"), "object": switch(),
        "property": "value", "path": "Main.Run" }),
    );
    assert_eq!(
        made["object"]["bind"]["value"],
        json!({ "path": "Main.Run", "direction": "read-write" })
    );
    let mut writing = switch();
    writing["bind"] = json!({ "value": { "path": "Main.Run", "direction": "write" } });
    let made = bind(json!({ "component": component("switch"), "object": writing,
        "property": "value", "path": "Main.Stop" }));
    assert_eq!(
        made["object"]["bind"]["value"],
        json!({ "path": "Main.Stop", "direction": "write" })
    );
    // A component the catalogue lacks binds for reading.
    let made = bind(json!({ "object": switch(), "property": "value", "path": "Main.Run" }));
    assert_eq!(made["object"]["bind"]["value"]["direction"], json!("read"));
}

#[test]
fn a_typed_path_binds_keeping_the_bindings_unit_and_an_empty_one_removes_it() {
    // Wrong behaviour caught: retyping a path dropping the binding's unit, or an emptied field
    // leaving a binding without a path.
    let mut shown = json!({ "id": "speed", "type": "value", "at": [40, 40], "size": [160, 48],
        "rotation": 0, "bind": { "value": { "path": "Main.Speed", "unit": "rpm" },
            "visible": { "path": "Main.Run" } } });
    let made = bind(json!({ "component": component("value"), "object": shown,
        "property": "value", "path": "Main.P101.Speed" }));
    assert_eq!(
        made["object"]["bind"]["value"],
        json!({ "path": "Main.P101.Speed", "unit": "rpm", "direction": "read" })
    );
    shown = made["object"].clone();
    let made = bind(json!({ "component": component("value"), "object": shown,
        "property": "value", "path": "" }));
    assert_eq!(
        made["object"]["bind"],
        json!({ "visible": { "path": "Main.Run", "direction": "read" } })
    );
}

#[test]
fn a_variable_dropped_on_a_binding_it_cannot_have_is_refused_in_the_binding_checks_words() {
    // §7.1: the binding stays unchanged and the editor says why. Wrong behaviour caught: a REAL
    // bound to a switch, a constant written by a numeric input, an output of a block written,
    // or a refusal without its reason.
    let refused = |component_kind: &str, object: Value, property: &str, node: Value| {
        let made = bind(
            json!({ "component": component(component_kind), "object": object,
            "property": property, "variable": node }),
        );
        assert_eq!(made.get("object"), None, "{made}");
        made["refused"].as_str().unwrap().to_string()
    };
    assert_eq!(
        refused("switch", switch(), "value", speed()),
        "\"Main.Speed\" is REAL; a switch's value needs a boolean"
    );
    let input = json!({ "id": "entry", "type": "numeric-input", "at": [0, 0], "size": [208, 48],
        "rotation": 0, "bind": {} });
    assert_eq!(
        refused("numeric-input", input.clone(), "value", limit()),
        "\"Main.Limit\" is a constant and cannot be written"
    );
    let mut level = value("Main.Level", "REAL", &["number", "value"]);
    level["writable"] = json!(false);
    assert_eq!(
        refused("numeric-input", input.clone(), "value", level),
        "\"Main.Level\" is read-only and cannot be written"
    );
    assert_eq!(
        refused("numeric-input", input, "colour", speed()),
        "a numeric input has no property \"colour\""
    );
    let made = bind(
        json!({ "component": component("switch"), "object": switch(),
        "property": "value", "variable": run() }),
    );
    assert_eq!(
        made["object"]["bind"]["value"],
        json!({ "path": "Main.Run", "direction": "read-write" }),
        "a writable boolean binds"
    );
}

fn dropped(variable: Value) -> Value {
    let components = serde_json::to_value(&Catalogue::shipped().components).unwrap();
    answer(json!({ "action": "drop", "components": components, "variable": variable }))
        .expect("an answer")
}

#[test]
fn a_variable_dropped_on_the_canvas_creates_the_matching_component() {
    // §7.1: a boolean creates a Status, a structure or an array a Table, any other single value
    // a Value, bound by its first property that accepts it. Wrong behaviour caught: a boolean
    // made into a Value, a structure into a Value that cannot show it, or an enumeration bound
    // to the Status (which the rule keeps for booleans).
    let made = |node: Value| {
        let made = dropped(node);
        let bind = made["bind"].as_object().expect("a binding");
        let (property, binding) = bind.iter().next().unwrap();
        format!(
            "{}.{property} {}",
            made["type"].as_str().unwrap(),
            binding["direction"].as_str().unwrap()
        )
    };
    assert_eq!(made(run()), "status.state read");
    assert_eq!(made(speed()), "value.value read");
    assert_eq!(
        made(value("Main.Name", "STRING", &["text", "value"])),
        "value.value read"
    );
    assert_eq!(
        made(value("Main.Mode", "Modes", &["enumeration", "value"])),
        "value.value read"
    );
    let mut reading = value("Main.Reading", "Reading", &["structure"]);
    reading["kind"] = json!("structure");
    assert_eq!(made(reading), "table.value read");
    let mut grid = value("Main.Grid", "ARRAY[1..2] OF INT", &["array"]);
    grid["kind"] = json!("array");
    assert_eq!(made(grid), "table.value read");
    assert_eq!(
        dropped(run())["bind"],
        json!({ "state": { "path": "Main.Run", "direction": "read" } })
    );
}

#[test]
fn an_object_made_from_a_variable_is_labelled_with_the_last_part_of_its_path() {
    // Wrong behaviour caught: the label holding the whole path, or an array element cut at its
    // index.
    let label = |path: &str| dropped(value(path, "BOOL", &["boolean", "value"]))["label"].clone();
    assert_eq!(label("Main.P101.Running"), json!("Running"));
    assert_eq!(label("Main.Pumps[2]"), json!("Pumps[2]"));
    assert_eq!(label("Main.Pumps[1].Running"), json!("Running"));
    assert_eq!(label("Main.Grid[1,0]"), json!("Grid[1,0]"));
}

#[test]
fn a_program_or_function_block_instance_dropped_on_the_canvas_says_to_drag_one_of_its_members() {
    // §7.1: instances create nothing (their symbols are §8). Wrong behaviour caught: an object
    // made for a block instance, or no word of what to drag instead.
    let refused = |node: Value| {
        let made = dropped(node);
        assert_eq!(made.get("type"), None, "{made}");
        made["refused"].clone()
    };
    assert_eq!(
        refused(p101()),
        json!("\"Main.P101\" is a function block instance (Pump): drag one of its members onto the page")
    );
    assert_eq!(
        refused(tree()[0].clone()),
        json!("\"Main\" is a program: drag one of its variables onto the page")
    );
    assert_eq!(
        refused(value("Main.Target", "REF_TO INT", &[])),
        json!("\"Main.Target\" (REF_TO INT) has no component to show it")
    );
}

#[test]
fn a_request_without_a_path_or_a_variable_is_refused() {
    // Wrong behaviour caught: a binding request that names nothing setting an empty binding.
    let refused = answer(json!({ "action": "bind", "object": switch(), "property": "value" }));
    assert_eq!(refused, Err("bind needs a path or a variable.".to_string()));
}
