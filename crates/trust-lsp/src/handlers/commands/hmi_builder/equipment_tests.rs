//! Equipment objects (specification 39, §6 item 5, §8.5, §18 phase 4 item 1, §21.3): the
//! extension's former `hmi-component-instances.test.ts` cases and the loop-signal case of
//! `hmi-builder-editing.test.ts`, moved here with the rules.

use serde_json::{json, Value};

use super::answer;

fn ask(action: &str, request: Value) -> Value {
    let mut request = request;
    request["action"] = json!(action);
    let reply = answer(request).expect("an answer");
    assert_eq!(reply["ok"], json!(true), "{reply}");
    reply
}

/// The Plant library's pump, as its component definition gives it (§8.5).
fn pump() -> Value {
    json!({ "block": "Plant.FB_Pump", "title": "Pump", "symbol": "pump", "panel": "plant-run",
        "variants": [], "bindings": { "running": "Running", "fault": "Fault" }, "fields": {},
        "settings": [], "io": [], "dependencies": [], "position": "actuators" })
}

fn object(id: &str, kind: &str) -> Value {
    json!({ "id": id, "type": kind, "at": [0, 0], "size": [128, 108], "rotation": 0, "bind": {} })
}

fn shown(id: &str, kind: &str, instance: &str) -> Value {
    let mut shown = object(id, kind);
    shown["instance"] = json!(instance);
    shown
}

fn block_instance(path: &str, type_name: &str) -> Value {
    json!({ "name": path.rsplit('.').next().unwrap(), "path": path, "kind": "function-block",
        "type": type_name, "kinds": [], "writable": false })
}

fn program(path: &str, children: Vec<Value>) -> Value {
    json!({ "name": path, "path": path, "kind": "program", "type": path, "kinds": [],
        "writable": false, "children": children })
}

fn page(objects: Vec<Value>) -> Value {
    json!({ "title": "Plant", "object": objects, "pipe": [] })
}

fn placed(variables: Value, page: Value) -> Value {
    ask(
        "place",
        json!({ "object": object("pump-1", "pump"), "definition": pump(), "variables": variables,
            "page": page }),
    )["object"]
        .clone()
}

#[test]
fn placement_avoids_source_and_page_instance_names_without_guessing_among_programs() {
    // `Main.Pump2` after `Main.Pump1` and an object holding `Main.Pump2`. Wrong behaviour
    // caught: two objects given one instance, a name the program declares reused, names compared
    // with their case, or one of several programs chosen for the user.
    let main = program("Main", vec![block_instance("Main.Pump1", "Plant.FB_Pump")]);
    let holding = page(vec![shown("pump-0", "pump", "Main.Pump2")]);
    assert_eq!(
        placed(json!([main]), holding.clone())["instance"],
        json!("Main.Pump3")
    );
    assert_eq!(
        placed(json!([]), holding)["instance"],
        json!("Main.Pump1"),
        "no program yet: Main"
    );
    let cased = program("Main", vec![block_instance("MAIN.PUMP1", "Plant.FB_Pump")]);
    assert_eq!(
        placed(
            json!([cased]),
            page(vec![shown("pump-0", "pump", "main.pump2")])
        )["instance"],
        json!("Main.Pump3")
    );
    let several = placed(
        json!([program("Main", vec![]), program("Other", vec![])]),
        page(vec![]),
    );
    assert_eq!(
        several.get("instance"),
        None,
        "the user chooses the program"
    );
    assert_eq!(several["block"], json!("Plant.FB_Pump"));
    assert_eq!(several["faceplate"], json!("plant-run"));
    assert_eq!(several["bind"], json!({}));
}

#[test]
fn a_screen_service_program_is_no_place_for_equipment() {
    // §18 phase 5: the program that calls `Hmi.Display` serves the screens. Wrong behaviour
    // caught: that program counted as a second program, so a new pump got no instance.
    let screens = program(
        "Screens",
        vec![block_instance("Screens.Display", "Hmi.Display")],
    );
    let placed = placed(json!([program("Line", vec![]), screens]), page(vec![]));
    assert_eq!(placed["instance"], json!("Line.Pump1"));
}

#[test]
fn rebinding_changes_relative_fields_once_and_keeps_click_disabled_until_requested() {
    // Wrong behaviour caught: the tag left at the old instance's name, the old bindings kept, or
    // a click that opens nothing turned into one that opens the panel.
    let bind = |object: Value, instance: &str| {
        ask(
            "bind",
            json!({ "object": object, "definition": pump(), "instance": instance }),
        )["object"]
            .clone()
    };
    let first = bind(object("pump-1", "pump"), "Main.P101");
    assert_eq!(
        first,
        json!({ "id": "pump-1", "type": "pump", "block": "Plant.FB_Pump", "instance": "Main.P101",
            "faceplate": "plant-run", "tag": "P101", "at": [0, 0], "size": [128, 108],
            "rotation": 0, "bind": {
                "running": { "path": "Main.P101.Running", "direction": "read" },
                "fault": { "path": "Main.P101.Fault", "direction": "read" } } })
    );
    let second = bind(first.clone(), "Main.P102");
    assert_eq!(second["tag"], json!("P102"));
    assert_eq!(
        second["bind"]["running"]["path"],
        json!("Main.P102.Running")
    );
    assert_eq!(second.get("click"), None);
    let mut opening = first.clone();
    opening["click"] = json!({ "faceplate": "plant-run", "instance": "Main.P101" });
    assert_eq!(
        bind(opening, "Main.P102")["click"],
        json!({ "faceplate": "plant-run", "instance": "Main.P102" })
    );
    let mut paging = first;
    paging["click"] = json!({ "page": "overview" });
    assert_eq!(
        bind(paging, "Main.P102")["click"],
        json!({ "page": "overview" })
    );
}

#[test]
fn a_pid_loops_symbol_binds_the_variables_beside_its_pid() {
    // §8.5, §10.5: `{instance}_Setpoint` of `Main.TIC101` is `Main.TIC101_Setpoint`, while a
    // plain member is a member of the instance. Wrong behaviour caught: `Main.TIC101.{instance}_…`
    // or `Main.TIC101._Setpoint`, and a quality member not following its property.
    let mut loop_definition = pump();
    loop_definition["block"] = json!("Control.PID");
    loop_definition["symbol"] = json!("controller");
    loop_definition["panel"] = json!("plant-pid");
    loop_definition["bindings"] = json!({ "pv": "{instance}_Input.ProcessValue",
        "sp": "{instance}_Setpoint", "manual": "{instance}_Manual" });
    loop_definition["quality"] = json!({ "pv": "{instance}_Input.Quality", "sp": "" });
    let bound = ask(
        "bind",
        json!({ "object": object("controller-1", "controller"), "definition": loop_definition,
            "instance": "Main.TIC101" }),
    )["object"]
        .clone();
    assert_eq!(
        bound["bind"],
        json!({
            "pv": { "path": "Main.TIC101_Input.ProcessValue", "quality": "Main.TIC101_Input.Quality",
                "direction": "read" },
            "sp": { "path": "Main.TIC101_Setpoint", "direction": "read" },
            "manual": { "path": "Main.TIC101_Manual", "direction": "read" },
        })
    );
    let mut quality = pump();
    quality["quality"] = json!({ "running": "Quality" });
    let bound = ask(
        "bind",
        json!({ "object": object("pump-1", "pump"), "definition": quality,
            "instance": "Main.P101" }),
    )["object"]
        .clone();
    assert_eq!(
        bound["bind"]["running"]["quality"],
        json!("Main.P101.Quality")
    );
}

#[test]
fn a_tag_is_checked_and_a_program_keeps_the_objects_name_or_gives_its_first_free_one() {
    // The inspector's tag and program fields (§21.3 item 2). Wrong behaviour caught: a tag with
    // a space or a dot, or an emptied tag, written as an instance; a program without a name; a new
    // object moved to a program binding to a name already taken; or an object moved to another
    // program losing its name.
    let bind = |object: Value, request: Value| {
        let mut request = request;
        request["object"] = object;
        request["definition"] = pump();
        ask("bind", request)
    };
    let new = || object("pump-1", "pump");
    let variables = json!([program(
        "Main",
        vec![block_instance("Main.Pump1", "Plant.FB_Pump")]
    )]);
    let free = bind(
        new(),
        json!({ "program": "Main", "variables": variables,
            "page": page(vec![shown("pump-0", "pump", "Main.Pump2")]) }),
    );
    assert_eq!(free["object"]["instance"], json!("Main.Pump3"));
    let moved = bind(
        shown("pump-1", "pump", "Main.P101"),
        json!({ "program": "Line2" }),
    );
    assert_eq!(moved["object"]["instance"], json!("Line2.P101"));
    assert_eq!(
        bind(new(), json!({ "program": "Main", "name": "Pump7" }))["object"]["instance"],
        json!("Main.Pump7")
    );
    for (request, refused) in [
        (
            json!({ "program": "Main", "name": "Pump 7" }),
            "\"Pump 7\" is not a name",
        ),
        (
            json!({ "program": "Main", "name": "A.B" }),
            "\"A.B\" is not a name",
        ),
        (
            json!({ "program": "Main", "name": "" }),
            "\"\" is not a name",
        ),
        (
            json!({ "program": "", "name": "Pump7" }),
            "\".Pump7\" is not an instance of a program",
        ),
        (
            json!({ "instance": "Pump7" }),
            "\"Pump7\" is not an instance of a program",
        ),
    ] {
        let answer = bind(new(), request);
        assert_eq!(answer.get("object"), None, "{answer}");
        assert_eq!(answer["refused"], json!(refused));
    }
}

#[test]
fn the_inspector_offers_the_declared_instances_of_the_block_and_the_programs() {
    // §21.3 item 2. Wrong behaviour caught: an instance of another block offered, a type
    // compared with its case, a pending design instance offered as declared, the screen service
    // program offered for equipment, or the program of a bound object guessed.
    let mut pending = block_instance("Main.P103", "Plant.FB_Pump");
    pending["pending"] = json!(true);
    let main = program(
        "Main",
        vec![
            block_instance("Main.P101", "Plant.FB_Pump"),
            block_instance("Main.P102", "plant.fb_pump"),
            block_instance("Main.V101", "Plant.FB_Valve"),
            pending,
        ],
    );
    let screens = program(
        "Screens",
        vec![block_instance("Screens.Display", "HMI.DISPLAY")],
    );
    let variables = json!([main, screens]);
    let view = ask(
        "instances",
        json!({ "block": "Plant.FB_Pump", "instance": "main.p101", "variables": variables }),
    );
    assert_eq!(view["declared"], json!(["Main.P101", "Main.P102"]));
    assert_eq!(view["current"], json!(true));
    assert_eq!(
        view["programs"],
        json!([{ "path": "Main", "name": "Main" }])
    );
    assert_eq!(
        (view["program"].clone(), view["name"].clone()),
        (json!("main"), json!("p101"))
    );
    let new = ask(
        "instances",
        json!({ "block": "Plant.FB_Pump", "variables": variables }),
    );
    assert_eq!(new["current"], json!(false));
    assert_eq!(
        (new["program"].clone(), new["name"].clone()),
        (json!("Main"), json!(""))
    );
    let none = ask(
        "instances",
        json!({ "block": "Plant.FB_Pump", "variables": [] }),
    );
    assert_eq!(
        none["program"],
        json!("Main"),
        "a project without a program yet"
    );
    let several = ask(
        "instances",
        json!({ "block": "Plant.FB_Pump",
            "variables": [program("Main", vec![]), program("Other", vec![])] }),
    );
    assert_eq!(several["program"], json!(""), "the user chooses");
}

fn component(kind: &str, category: &str) -> Value {
    json!({ "type": kind, "category": category, "phase": 2, "size": [128, 108] })
}

fn palette(catalogue: Value, pid: bool) -> Vec<String> {
    let reply = ask("palette", json!({ "catalogue": catalogue, "pid": pid }));
    reply["palette"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| match entry["equipment"]["block"].as_str() {
            Some(block) => format!("{}:{block}", entry["type"].as_str().unwrap()),
            None => entry["type"].as_str().unwrap().to_string(),
        })
        .collect()
}

#[test]
fn multiple_fb_variants_share_a_renderer_without_disappearing_from_the_palette() {
    // §21.3 item 1. Wrong behaviour caught: the second definition of one drawing hidden, or the
    // drawing itself also offered as a symbol without a block.
    let mut drive = pump();
    drive["block"] = json!("Plant.FB_VariableSpeedPump");
    drive["title"] = json!("Variable-speed pump");
    let catalogue = json!({ "format": 1, "components": [component("pump", "equipment")],
        "equipment": [pump(), drive] });
    assert_eq!(
        palette(catalogue.clone(), false),
        ["pump:Plant.FB_Pump", "pump:Plant.FB_VariableSpeedPump"]
    );
    let reply = ask("palette", json!({ "catalogue": catalogue }));
    let entry = &reply["palette"][1];
    assert_eq!(entry["category"], json!("equipment"));
    assert_eq!(entry["equipment"]["title"], json!("Variable-speed pump"));
    assert_eq!(entry["size"], json!([128, 108]));
}

#[test]
fn equipment_with_a_definition_comes_first_and_unmapped_artwork_stays_usable() {
    // §21.3 item 1: each definition with its drawing, then the general elements. Wrong
    // behaviour caught: artwork without a definition lost, a display that a definition draws
    // with taken out of the displays, or an entry without its definition.
    let definition = |block: &str, symbol: &str| {
        let mut definition = pump();
        definition["block"] = json!(block);
        definition["symbol"] = json!(symbol);
        definition
    };
    let catalogue = json!({ "format": 1,
        "components": [component("pump", "equipment"), component("level-sensor", "equipment"),
            component("controller", "equipment"), component("value", "display"),
            component("mixer", "equipment"), component("panel", "layout"),
            component("page-button", "layout")],
        "equipment": [definition("Plant.FB_Pump", "pump"),
            definition("Plant.FB_LevelInput", "level-sensor"),
            definition("Control.PID", "controller"),
            definition("Plant.FB_DutyStandbyGroup", "value"),
            definition("Plant.FB_Missing", "no-such-drawing")] });
    assert_eq!(
        palette(catalogue, false),
        [
            "pump:Plant.FB_Pump",
            "level-sensor:Plant.FB_LevelInput",
            "controller:Control.PID",
            "value:Plant.FB_DutyStandbyGroup",
            "value",
            "mixer",
            "panel",
            "page-button",
        ]
    );
}

#[test]
fn a_pid_offers_what_a_pid_may_hold() {
    // §18 phase 4 item 1: process objects, texts and lines. Wrong behaviour caught: displays,
    // controls or panels offered on a P&ID, or its equipment definitions lost.
    let catalogue = json!({ "format": 1,
        "components": [component("pump", "equipment"), component("junction", "equipment"),
            component("value", "display"), component("switch", "control"),
            component("text", "display"), component("line", "layout"),
            component("panel", "layout")],
        "equipment": [pump()] });
    assert_eq!(
        palette(catalogue.clone(), true),
        ["pump:Plant.FB_Pump", "junction", "text", "line"]
    );
    assert_eq!(
        palette(catalogue, false),
        [
            "pump:Plant.FB_Pump",
            "junction",
            "value",
            "switch",
            "text",
            "line",
            "panel"
        ]
    );
}

#[test]
fn an_objects_definition_is_found_by_its_block_without_regard_to_case() {
    // IEC names do not depend on case. Wrong behaviour caught: an object recording
    // `plant.fb_pump` shown without its equipment inspector, or a block without a definition
    // given another's.
    let reply = ask(
        "definitions",
        json!({ "equipment": [pump()], "blocks": ["plant.fb_pump", "Plant.FB_Valve"] }),
    );
    assert_eq!(reply["definitions"], json!({ "plant.fb_pump": pump() }));
}

fn signals(object: Value, page: Value, dependencies: Value) -> Value {
    ask(
        "loopSignals",
        json!({ "object": object, "page": page, "dependencies": dependencies }),
    )
}

#[test]
fn a_loop_controllers_lines_follow_its_configured_measurement_and_controlled_equipment() {
    // §6 item 5. Wrong behaviour caught: a line to an instance the page does not show, lines
    // that do not follow a changed configuration, instances compared with their case, or a
    // change reported when the lines are already right.
    let controller = shown("tic", "controller", "Main.TIC101");
    let page = page(vec![
        controller.clone(),
        shown("tt", "temperature-sensor", "Main.TT101"),
        shown("eh", "heater", "Main.EH101"),
    ]);
    let configured = json!([
        { "member": "Input", "drives": false, "value": "Main.TT101" },
        { "member": "Output", "drives": true, "value": "main.eh101" },
    ]);
    let lined = signals(controller, page.clone(), configured.clone())["object"].clone();
    assert_eq!(
        (lined["measures"].clone(), lined["acts_on"].clone()),
        (json!("tt"), json!("eh"))
    );
    let unchanged = signals(lined.clone(), page.clone(), configured);
    assert_eq!(unchanged.get("object"), None, "nothing to change");
    let elsewhere = signals(
        lined,
        page.clone(),
        json!([{ "drives": false, "value": "Main.TT101" }, { "drives": true, "value": "Main.TV101" }]),
    )["object"]
        .clone();
    assert_eq!(
        elsewhere.get("acts_on"),
        None,
        "no line to equipment the page does not show"
    );
    assert_eq!(elsewhere["measures"], json!("tt"));
}

#[test]
fn a_loop_controllers_line_goes_to_the_first_connected_equipment_with_an_instance() {
    // A connection not made yet (empty or null) is no line. Wrong behaviour caught: the
    // measurement line dropped because the first measurement listed is not connected.
    let page = page(vec![shown("tt", "temperature-sensor", "Main.TT101")]);
    let lined = signals(
        shown("tic", "controller", "Main.TIC101"),
        page,
        json!([{ "value": "" }, { "drives": null, "value": null },
            { "drives": false, "value": "Main.TT101" }]),
    )["object"]
        .clone();
    assert_eq!(lined["measures"], json!("tt"));
    assert_eq!(lined.get("acts_on"), None);
}
