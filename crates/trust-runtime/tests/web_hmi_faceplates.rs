//! Specification 39, §8.7: a faceplate opens bound to the instance its page object names, its
//! fields becoming the instance's members by its pair; it streams their values and sends their
//! commands; a library's faceplate serves when the project has none of that name.

use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::hmi::access::config::{hmi_access_from_runtime_toml, HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::security::AccessRole;
use tungstenite::Message;

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

const PROGRAM: &str = r#"
FUNCTION_BLOCK Pump
VAR_INPUT
    StartCmd : BOOL;
END_VAR
VAR_OUTPUT
    Running : BOOL;
    ActualSpeed : REAL;
END_VAR
    Running := StartCmd;
    IF Running THEN
        ActualSpeed := 1460.0;
    ELSE
        ActualSpeed := 0.0;
    END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    P101 : Pump;
    P102 : Pump;
END_VAR
    P101();
    P102(StartCmd := TRUE);
END_PROGRAM
"#;

const PAGE: &str = r#"format = 1
title = "Pumps"

[[object]]
id = "pump-1"
type = "pump"
at = [40, 40]
size = [128, 108]

[object.bind]
running = "Main.P101.Running"

[object.click]
faceplate = "pump"

[[object]]
id = "pump-2"
tag = "P-102"
type = "pump"
at = [240, 40]
size = [128, 108]

[object.click]
faceplate = "pump"
instance = "Main.P102"

[[object]]
id = "fan-1"
tag = "F-101"
type = "fan"
variant = "variable-speed"
at = [440, 40]
size = [128, 108]

[object.click]
faceplate = "pump"
instance = "Main.P102"

[[object]]
id = "label"
type = "text"
at = [40, 200]
size = [160, 32]
text = "Pumps"
"#;

const FACEPLATE: &str = r#"format = 1
title = "Pump"
description = "Transfer pump"
size = [400, 300]

[fields.Speed]
accepts = ["number"]

[fields.StartCmd]
accepts = ["boolean"]

[[object]]
id = "symbol"
type = "pump"
at = [240, 24]
size = [128, 108]

[[object]]
id = "speed"
type = "value"
at = [24, 24]
size = [200, 48]

[object.bind]
value = "$Speed"

[[object]]
id = "start"
type = "button"
at = [24, 100]
size = [120, 44]
action = "set"

[object.bind]
target = { path = "$StartCmd", direction = "write" }
"#;

const PAIR: &str = "format = 1\nblock = \"Pump\"\nfaceplate = \"pump\"\nsymbol = \"pump\"\n\n[fields]\nSpeed = \"ActualSpeed\"\n";

fn get(base: &str, path: &str) -> (u16, Value) {
    let (status, body, _) = http_request("GET", base, path, None, None);
    (status, body)
}

fn faceplate_project(name: &str) -> std::path::PathBuf {
    project(
        name,
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", PAGE),
            ("hmi/panels/pump.toml", FACEPLATE),
            ("hmi/pairs/pump.toml", PAIR),
        ],
    )
}

fn paths(model: &Value) -> Vec<String> {
    model["object"]
        .as_array()
        .expect("objects")
        .iter()
        .flat_map(|object| {
            object["bind"].as_object().into_iter().flat_map(|bind| {
                bind.values()
                    .map(|binding| binding["path"].as_str().unwrap_or_default().to_string())
            })
        })
        .collect()
}

/// Specification 39, §5.9 item 5: a faceplate says which role each of its controls needs for the
/// instance it opens on.
#[test]
fn a_faceplate_says_which_role_each_control_needs_for_its_instance() {
    let access = HmiAccessConfig {
        write: hmi_access_from_runtime_toml(
            "[[runtime.web.hmi.write]]\npath = \"Main.P101.StartCmd\"\nrole = \"engineer\"\n",
        )
        .expect("the policy")
        .write,
        ..HmiAccessConfig::default()
    };
    let running = start_with(
        &faceplate_project("faceplate-roles"),
        WebAuthMode::Local,
        None,
        access,
        PROGRAM,
        None,
    );
    let one = get(&running.base, "/api/hmi/popup?faceplate=overview/pump-1").1;
    assert_eq!(one["popup"]["roles"]["start"], json!("engineer"), "{one}");
    let two = get(&running.base, "/api/hmi/popup?faceplate=overview/pump-2").1;
    assert!(
        two["popup"]["roles"]
            .get("start")
            .is_some_and(Value::is_null),
        "P102's command is not in the policy: {two}"
    );
}

#[test]
fn a_faceplate_opens_bound_to_the_instance_its_object_names_with_its_pairs_members() {
    let running = start_with(
        &faceplate_project("faceplate-open"),
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    let (status, data) = get(&running.base, "/api/hmi/popup?faceplate=overview/pump-1");
    assert_eq!(status, 200, "{data}");
    assert_eq!(data["popup"]["name"], json!("faceplate:overview/pump-1"));
    let model = &data["popup"]["model"];
    assert_eq!(
        model["title"],
        json!("P101 — Pump"),
        "the instance's own name, with the component's title after it"
    );
    assert_eq!(model["description"], json!("Transfer pump"));
    assert_eq!(
        paths(model),
        ["Main.P101.ActualSpeed", "Main.P101.StartCmd"],
        "the instance from the object's bindings, Speed by the pair"
    );
    let explicit = get(&running.base, "/api/hmi/popup?faceplate=overview/pump-2").1;
    assert_eq!(
        paths(&explicit["popup"]["model"]),
        ["Main.P102.ActualSpeed", "Main.P102.StartCmd"],
        "the instance the click names"
    );
    for nothing in [
        "overview/label",
        "overview/none",
        "elsewhere/pump-1",
        "pump-1",
    ] {
        assert_eq!(
            get(
                &running.base,
                &format!("/api/hmi/popup?faceplate={nothing}")
            )
            .0,
            404,
            "{nothing}"
        );
    }
}

/// Specification 39, §8.4 item 1: the title is the opening object's tag with the component's title
/// after it; the panel's own description stays below.
#[test]
fn a_faceplate_is_titled_by_the_tag_of_the_object_that_opens_it_with_its_description_below() {
    let running = start_with(
        &faceplate_project("faceplate-tag"),
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    let model =
        get(&running.base, "/api/hmi/popup?faceplate=overview/pump-2").1["popup"]["model"].clone();
    assert_eq!(
        (model["title"].clone(), model["description"].clone()),
        (json!("P-102 — Pump"), json!("Transfer pump"))
    );
    assert_eq!(
        model["object"][0]["tag"],
        json!("P-102"),
        "the faceplate's symbol takes the tag"
    );
    // A block's own type shares its base's faceplate and shows its own symbol there (§8.7 item 7).
    let fan =
        get(&running.base, "/api/hmi/popup?faceplate=overview/fan-1").1["popup"]["model"].clone();
    assert_eq!(
        (
            fan["object"][0]["type"].clone(),
            fan["object"][0]["variant"].clone(),
            fan["object"][0]["tag"].clone()
        ),
        (json!("fan"), json!("variable-speed"), json!("F-101"))
    );
    // Without a description of its own, the title alone names the equipment; a component
    // definition's title is the component's title.
    let plain = FACEPLATE.replace("description = \"Transfer pump\"\n", "");
    let root = project(
        "faceplate-tag-plain",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", PAGE),
            ("hmi/panels/pump.toml", &plain),
            ("hmi/pairs/pump.toml", PAIR),
        ],
    );
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    let model =
        get(&running.base, "/api/hmi/popup?faceplate=overview/pump-2").1["popup"]["model"].clone();
    assert_eq!(
        (model["title"].clone(), model["description"].clone()),
        (json!("P-102 — Pump"), Value::Null)
    );
    let titled = project(
        "faceplate-tag-component",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", PAGE),
            ("hmi/panels/pump.toml", &plain),
            (
                "hmi/pairs/pump.toml",
                &PAIR.replace("format = 1\n", "format = 1\ntitle = \"Transfer pump\"\n"),
            ),
        ],
    );
    let running = start_with(
        &titled,
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    let model =
        get(&running.base, "/api/hmi/popup?faceplate=overview/pump-2").1["popup"]["model"].clone();
    assert_eq!(model["title"], json!("P-102 — Transfer pump"));
}

#[test]
fn a_faceplate_shows_the_value_its_object_shows_in_the_objects_unit() {
    // The object that opens the faceplate hands its unit to the faceplate's objects bound to the
    // same variable, and its decimals where they have none of their own (§8.7 item 6).
    let page = PAGE.replace(
        "[[object]]\nid = \"pump-2\"",
        "[[object]]\nid = \"speed-1\"\ntag = \"SI-101\"\ntype = \"value\"\nat = [40, 300]\nsize = [160, 48]\nunit = \"rpm\"\ndecimals = 1\n\n[object.bind]\nvalue = \"Main.P101.ActualSpeed\"\n\n[object.click]\nfaceplate = \"pump\"\n\n[[object]]\nid = \"pump-2\"",
    );
    let root = project(
        "faceplate-unit",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", &page),
            ("hmi/panels/pump.toml", FACEPLATE),
            ("hmi/pairs/pump.toml", PAIR),
        ],
    );
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    let model =
        get(&running.base, "/api/hmi/popup?faceplate=overview/speed-1").1["popup"]["model"].clone();
    let object = |id: &str| {
        model["object"]
            .as_array()
            .and_then(|objects| objects.iter().find(|object| object["id"] == id))
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(
        (
            object("speed")["unit"].clone(),
            object("speed")["decimals"].clone()
        ),
        (json!("rpm"), json!(1))
    );
    assert_eq!(
        object("start")["unit"],
        Value::Null,
        "an object showing another variable keeps no unit"
    );
}

#[test]
fn a_faceplates_trend_takes_the_objects_unit_pen_by_pen() {
    // A trend's pens may show values of different units: only the pen bound to the variable the
    // opening object shows takes its unit (§8.7 item 6).
    let page = PAGE.replace(
        "[[object]]\nid = \"pump-2\"",
        "[[object]]\nid = \"speed-1\"\ntag = \"SI-101\"\ntype = \"value\"\nat = [40, 300]\nsize = [160, 48]\nunit = \"rpm\"\ndecimals = 1\n\n[object.bind]\nvalue = \"Main.P101.ActualSpeed\"\n\n[object.click]\nfaceplate = \"pump\"\n\n[[object]]\nid = \"pump-2\"",
    );
    let faceplate = format!(
        "{FACEPLATE}\n[fields.Running]\naccepts = [\"boolean\"]\n\n[[object]]\nid = \"history\"\ntype = \"trend\"\nat = [24, 160]\nsize = [360, 120]\n\n[object.bind]\nspeed = \"$Speed\"\nrunning = \"$Running\"\n"
    );
    let root = project(
        "faceplate-trend-unit",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", &page),
            ("hmi/panels/pump.toml", &faceplate),
            ("hmi/pairs/pump.toml", PAIR),
        ],
    );
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    let model =
        get(&running.base, "/api/hmi/popup?faceplate=overview/speed-1").1["popup"]["model"].clone();
    let history = model["object"]
        .as_array()
        .and_then(|objects| objects.iter().find(|object| object["id"] == "history"))
        .cloned()
        .unwrap_or_default();
    assert_eq!(history["bind"]["speed"]["unit"], json!("rpm"));
    assert_eq!(
        (
            history["bind"]["running"]["unit"].clone(),
            history["unit"].clone()
        ),
        (Value::Null, Value::Null),
        "a pen showing another variable keeps its own unit, and the trend has none"
    );
}

#[test]
fn a_faceplates_stream_carries_its_instances_values() {
    let running = start_with(
        &faceplate_project("faceplate-stream"),
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    let address = running.base.trim_start_matches("http://").to_string();
    let stream = TcpStream::connect(&address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("timeout");
    let (mut socket, _) = tungstenite::client(
        format!("ws://{address}/ws/hmi?faceplate=overview/pump-2"),
        stream,
    )
    .expect("the faceplate's stream opens");
    loop {
        let Message::Text(text) = socket.read().expect("a message") else {
            continue;
        };
        let message: Value = serde_json::from_str(text.as_str()).expect("JSON");
        if message["values"]["Main.P102.ActualSpeed"]["value"] == json!(1460.0) {
            break;
        }
    }
}

#[test]
fn a_command_from_a_faceplate_writes_its_instances_member() {
    let access = HmiAccessConfig {
        users: vec![HmiUser {
            name: "anna".into(),
            role: AccessRole::Operator,
            password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
                .expect("hash"),
        }],
        write: hmi_access_from_runtime_toml(
            "[[runtime.web.hmi.write]]\npath = \"Main.P101.StartCmd\"\nrole = \"operator\"\n",
        )
        .expect("policy")
        .write,
        ..HmiAccessConfig::default()
    };
    let running = start_with(
        &faceplate_project("faceplate-command"),
        WebAuthMode::Local,
        None,
        access,
        PROGRAM,
        None,
    );
    let cookie = session_cookie(&running.base, "anna");
    let bundle = get(&running.base, "/api/hmi/popup?faceplate=overview/pump-1").1["bundle"].clone();
    let body = json!({ "id": "f1", "bundle": bundle, "page": "faceplate:overview/pump-1", "object": "start", "action": "set" });
    let (status, answer, _) = http_request(
        "POST",
        &running.base,
        "/api/hmi/command",
        Some(&cookie),
        Some(body),
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["outcome"], json!("applied"), "{answer}");
    assert_eq!(answer["readback"], json!(true), "{answer}");
}

#[test]
fn a_librarys_faceplate_serves_when_the_project_has_none_and_the_projects_own_takes_its_place() {
    let library_faceplate = FACEPLATE.replace("title = \"Pump\"", "title = \"Library pump\"");
    let root = project(
        "faceplate-library",
        &[
            (
                "trust-lsp.toml",
                "[dependencies]\nequipment = { path = \"lib/equipment\" }\n",
            ),
            (
                "lib/equipment/trust-lsp.toml",
                "[package]\nversion = \"0.1.0\"\n",
            ),
            // A library has its sources; this one's are not needed here.
            (
                "lib/equipment/src/equipment.st",
                "(* The library's function blocks. *)\n",
            ),
            ("lib/equipment/hmi/panels/pump.toml", &library_faceplate),
            ("lib/equipment/hmi/pairs/pump.toml", PAIR),
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", PAGE),
        ],
    );
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    let shipped = get(&running.base, "/api/hmi/popup?faceplate=overview/pump-1").1;
    assert_eq!(
        shipped["popup"]["model"]["title"],
        json!("P101 — Library pump"),
        "{shipped}"
    );
    assert_eq!(
        paths(&shipped["popup"]["model"])[0],
        "Main.P101.ActualSpeed",
        "the library's pair"
    );
    std::fs::create_dir_all(root.join("hmi/panels")).expect("folder");
    std::fs::write(
        root.join("hmi/panels/pump.toml"),
        FACEPLATE.replace("title = \"Pump\"", "title = \"Project pump\""),
    )
    .expect("write");
    let own = get(&running.base, "/api/hmi/popup?faceplate=overview/pump-1").1;
    assert_eq!(
        own["popup"]["model"]["title"],
        json!("P101 — Project pump"),
        "{own}"
    );
}
