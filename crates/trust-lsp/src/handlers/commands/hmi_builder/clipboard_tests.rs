//! Copy, paste and duplicate (specification 39, §3.3 items 6 and 8, §6 item 5): the rules the
//! extension's `hmi-builder-webview-editing.test.ts` cases exercised through its webview, here
//! on the language server's answer.

use serde_json::{json, Value};

use super::answer;

fn object(id: &str, kind: &str, at: [i64; 2], extra: Value) -> Value {
    let mut object =
        json!({ "id": id, "type": kind, "at": at, "size": [128, 108], "rotation": 0, "bind": {} });
    if let Value::Object(fields) = extra {
        for (key, value) in fields {
            object[key] = value;
        }
    }
    object
}

fn read(path: &str) -> Value {
    json!({ "path": path, "direction": "read" })
}

fn page(objects: Vec<Value>, pipes: Value) -> Value {
    json!({ "title": "Overview", "object": objects, "pipe": pipes })
}

fn ask(action: &str, request: Value) -> Value {
    let mut request = request;
    request["action"] = json!(action);
    let reply = answer(request).expect("an answer");
    assert_eq!(reply["ok"], json!(true), "{reply}");
    reply
}

fn copy(page: &Value, ids: &[&str]) -> String {
    ask("copy", json!({ "page": page, "ids": ids }))["text"]
        .as_str()
        .expect("clipboard text")
        .to_string()
}

fn paste(page: &Value, text: &str, step: [i64; 2], variables: Value) -> Value {
    ask(
        "paste",
        json!({ "page": page, "text": text, "step": step, "variables": variables }),
    )
}

fn variable(path: &str, children: Vec<Value>) -> Value {
    json!({ "name": path.rsplit('.').next().unwrap(), "path": path,
        "kind": if children.is_empty() { "value" } else { "function-block" },
        "type": "REAL", "kinds": ["number", "value"], "writable": false, "children": children })
}

fn overview() -> Value {
    page(
        vec![
            object(
                "speed",
                "value",
                [40, 40],
                json!({ "size": [240, 96], "label": "Pump speed",
                    "bind": { "value": read("Main.P101.Speed") } }),
            ),
            object("level", "bar", [320, 40], json!({ "size": [80, 240] })),
        ],
        json!([]),
    )
}

#[test]
fn a_copy_pastes_with_a_new_id_its_properties_and_bindings_next_to_the_original() {
    // Wrong behaviour caught: the copy keeping the original's id, losing its label or binding,
    // landing on the original, or the original changed.
    let page = overview();
    let pasted = paste(&page, &copy(&page, &["speed"]), [16, 16], json!([]));
    assert_eq!(pasted["ids"], json!(["value-1"]));
    let objects = pasted["page"]["object"].as_array().unwrap();
    assert_eq!(objects.len(), 3);
    assert_eq!(objects[..2], page["object"].as_array().unwrap()[..]);
    assert_eq!(
        objects[2],
        object(
            "value-1",
            "value",
            [56, 56],
            json!({ "size": [240, 96], "label": "Pump speed",
                "bind": { "value": read("Main.P101.Speed") } }),
        )
    );
    assert_eq!(pasted["page"]["title"], json!("Overview"));
}

#[test]
fn a_copy_gets_the_first_free_id_of_its_type() {
    // §3.3 item 6. Wrong behaviour caught: a pasted id that another object of the page holds,
    // or the second copy given the same id as the first.
    let page = page(
        vec![
            object("value-1", "value", [0, 0], json!({})),
            object("value-3", "value", [0, 100], json!({})),
        ],
        json!([]),
    );
    let pasted = paste(
        &page,
        &copy(&page, &["value-1", "value-3"]),
        [16, 16],
        json!([]),
    );
    assert_eq!(pasted["ids"], json!(["value-2", "value-4"]));
}

#[test]
fn a_copied_binding_moves_to_the_next_number_when_that_variable_exists() {
    // §3.3 item 8, smart numbering. Wrong behaviour caught: a binding renumbered to a variable
    // the program does not have (`P102.Running`), zero padding lost (`TT009` to `TT10`), the
    // program's variables compared with their case, or a pipe's flow left behind.
    let variables = json!([variable(
        "Main",
        vec![
            variable("Main.TT101", vec![]),
            variable("Main.TT102", vec![]),
            variable("Main.TT010", vec![]),
            variable("Main.FT102", vec![]),
            variable("Main.P101", vec![variable("Main.P101.Running", vec![])]),
        ],
    )]);
    let page = page(
        vec![
            object(
                "tt",
                "value",
                [40, 40],
                json!({ "bind": { "value": read("Main.TT101") } }),
            ),
            object(
                "pump",
                "pump",
                [40, 200],
                json!({ "bind": { "running": read("Main.P101.Running") } }),
            ),
            object(
                "tt9",
                "value",
                [240, 40],
                json!({ "bind": { "value": read("main.tt009") } }),
            ),
            object(
                "ft",
                "flow-sensor",
                [240, 200],
                json!({ "bind": { "value": read("Main.FT7") } }),
            ),
        ],
        json!([{ "from": "tt.outlet", "to": "pump.inlet", "bind": { "flow": read("Main.FT101") } }]),
    );
    let pasted = paste(
        &page,
        &copy(&page, &["tt", "pump", "tt9", "ft"]),
        [16, 16],
        variables,
    );
    let paths: Vec<Value> = pasted["page"]["object"].as_array().unwrap()[4..]
        .iter()
        .map(|object| object["bind"].as_object().unwrap().values().next().unwrap()["path"].clone())
        .collect();
    assert_eq!(
        paths,
        [
            json!("Main.TT102"),
            json!("Main.P101.Running"),
            json!("main.tt010"),
            json!("Main.FT7"),
        ]
    );
    assert_eq!(
        pasted["page"]["pipe"][1]["bind"]["flow"]["path"],
        json!("Main.FT102")
    );
}

#[test]
fn pipes_between_copied_objects_come_along_and_pipes_to_objects_not_copied_are_dropped() {
    // §3.3 item 8. Wrong behaviour caught: a pipe to an object left behind pasted dangling, a
    // pasted pipe still joining the originals, or its bends left where the originals' are.
    let page = page(
        vec![
            object("p1", "pump", [40, 40], json!({})),
            object("t1", "tank", [400, 40], json!({ "size": [160, 200] })),
            object("v1", "value", [700, 40], json!({ "size": [200, 80] })),
        ],
        json!([
            { "from": "p1.outlet", "to": "t1.inlet", "waypoints": [[300, 94]] },
            { "from": "t1.outlet", "to": "v1.inlet" },
        ]),
    );
    let pasted = paste(&page, &copy(&page, &["p1", "t1"]), [16, 16], json!([]));
    assert_eq!(pasted["ids"], json!(["pump-1", "tank-1"]));
    assert_eq!(
        pasted["page"]["pipe"].as_array().unwrap()[2..],
        [json!({ "from": "pump-1.outlet", "to": "tank-1.inlet", "waypoints": [[316, 110]] })]
    );
}

#[test]
fn an_instrument_keeps_what_it_measures_only_when_that_came_along() {
    // §3.3 item 8, §6 item 5. Wrong behaviour caught: a pasted instrument attached to the
    // original tank or to a pipe of the originals, or to a pipe the copy did not bring.
    let page = page(
        vec![
            object("t1", "tank", [400, 40], json!({})),
            object("v1", "value", [700, 40], json!({})),
            object("p1", "pump", [40, 40], json!({})),
            object("s1", "level-sensor", [300, 0], json!({ "attach": "t1" })),
            object(
                "s2",
                "pressure-sensor",
                [200, 0],
                json!({ "attach": "pipe:1" }),
            ),
        ],
        json!([
            { "from": "t1.outlet", "to": "v1.inlet" },
            { "from": "p1.outlet", "to": "t1.inlet" },
        ]),
    );
    let text = copy(&page, &["s2", "s1", "p1", "t1"]);
    let clipboard: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        clipboard["objects"][3]["attach"],
        json!("pipe:0"),
        "its place among the copied"
    );
    let pasted = paste(&page, &text, [16, 16], json!([]));
    assert_eq!(
        pasted["ids"],
        json!(["tank-1", "pump-1", "level-sensor-1", "pressure-sensor-1"])
    );
    let objects = &pasted["page"]["object"];
    assert_eq!(objects[7]["attach"], json!("tank-1"));
    assert_eq!(
        objects[8]["attach"],
        json!("pipe:2"),
        "the pasted pipe, after the page's two"
    );
    assert_eq!(
        pasted["page"]["pipe"][2],
        json!({ "from": "pump-1.outlet", "to": "tank-1.inlet" })
    );
    let alone = paste(&page, &copy(&page, &["s1", "s2"]), [16, 16], json!([]));
    let objects = &alone["page"]["object"];
    assert_eq!(objects[5].get("attach"), None, "its tank stayed behind");
    assert_eq!(objects[6].get("attach"), None, "its pipe stayed behind");
}

#[test]
fn the_copy_is_marked_text_that_names_its_pipes_by_their_place_among_the_copied() {
    // Wrong behaviour caught: clipboard text another program cannot tell from ours, or a copy
    // of objects that are not on the page.
    let page = overview();
    let clipboard: Value = serde_json::from_str(&copy(&page, &["level", "speed"])).unwrap();
    assert_eq!(clipboard["trust-hmi-objects"], json!(1));
    assert_eq!(
        clipboard["objects"][0]["id"],
        json!("speed"),
        "in page order"
    );
    assert_eq!(clipboard["objects"][1]["id"], json!("level"));
    assert_eq!(clipboard["pipes"], json!([]));
    let nothing = ask("copy", json!({ "page": page, "ids": ["missing"] }));
    assert_eq!(nothing.get("text"), None);
}

#[test]
fn text_that_holds_no_objects_pastes_nothing() {
    // Wrong behaviour caught: any text on the clipboard pasted as objects, or an empty copy
    // reported as a change.
    let page = overview();
    let speed = page["object"][0].clone();
    for text in [
        "Pump speed".to_string(),
        json!({ "trust-hmi-objects": 1, "objects": [] }).to_string(),
        json!({ "objects": [speed] }).to_string(),
        json!({ "trust-hmi-objects": 2, "objects": [speed] }).to_string(),
    ] {
        let reply = paste(&page, &text, [16, 16], json!([]));
        assert_eq!(reply.get("page"), None, "{text}");
    }
    let broken = answer(json!({ "action": "paste", "page": page,
        "text": json!({ "trust-hmi-objects": 1, "objects": [{ "id": "x" }] }).to_string(),
        "step": [16, 16] }));
    assert!(
        broken
            .unwrap_err()
            .starts_with("The copied objects do not load:"),
        "marked text whose objects do not load is refused with the reason"
    );
}

#[test]
fn duplicate_pastes_a_copy_at_once_with_the_step_given() {
    // §3.3 item 8: Ctrl+D; the step is the last duplicate's. Wrong behaviour caught: the
    // duplicate ignoring the step it was given.
    let page = overview();
    let duplicated = ask(
        "duplicate",
        json!({ "page": page, "ids": ["speed"], "step": [48, 16], "variables": [] }),
    );
    assert_eq!(duplicated["ids"], json!(["value-1"]));
    assert_eq!(duplicated["page"]["object"][2]["at"], json!([88, 56]));
    let none = ask(
        "duplicate",
        json!({ "page": page, "ids": [], "step": [16, 16] }),
    );
    assert_eq!(none.get("page"), None);
}

#[test]
fn a_panel_keeps_its_size_and_fields_when_objects_are_pasted_on_it() {
    // The editor also edits popups and panels (§4.9, §8.7). Wrong behaviour caught: a paste
    // that drops a panel's size or fields, so the panel no longer loads as one.
    let mut panel = overview();
    panel["size"] = json!([400, 300]);
    panel["fields"] = json!({ "Running": { "accepts": ["boolean"] } });
    let pasted = paste(&panel, &copy(&panel, &["level"]), [16, 16], json!([]));
    assert_eq!(pasted["page"]["size"], json!([400, 300]));
    assert_eq!(
        pasted["page"]["fields"],
        json!({ "Running": { "accepts": ["boolean"] } })
    );
    assert_eq!(pasted["ids"], json!(["bar-1"]));
}
