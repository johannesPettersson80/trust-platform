//! Specification 39, §8.9: a faceplate placed on a page comes with the page, bound to the instance
//! its object names exactly as its popup is, with the role each of its controls needs; the page's
//! stream carries its values, its controls send their commands as the popup's do, and one change
//! to the faceplate file changes every placement.

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
    P103 : Pump;
END_VAR
    P101();
    P102(StartCmd := TRUE);
    P103();
END_PROGRAM
"#;

const PAGE: &str = r#"format = 1
title = "Pumps"

[[object]]
id = "card-1"
type = "faceplate"
faceplate = "pump"
instance = "Main.P101"
at = [40, 40]
size = [400, 300]

[[object]]
id = "card-2"
tag = "P-102"
type = "faceplate"
faceplate = "pump"
instance = "Main.P102"
at = [460, 40]
size = [400, 300]

[[object]]
id = "card-3"
type = "faceplate"
faceplate = "pump"
instance = "Main.P103"
at = [880, 40]
size = [200, 150]

[[object]]
id = "card-4"
type = "faceplate"
faceplate = "gone"
instance = "Main.P103"
at = [880, 240]
size = [200, 150]
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

fn get(base: &str, path: &str) -> Value {
    let (status, body, _) = http_request("GET", base, path, None, None);
    assert_eq!(status, 200, "{path}: {body}");
    body
}

fn placed_project(name: &str) -> std::path::PathBuf {
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
        .into_iter()
        .flatten()
        .flat_map(|object| {
            object["bind"].as_object().into_iter().flat_map(|bind| {
                bind.values()
                    .map(|binding| binding["path"].as_str().unwrap_or_default().to_string())
            })
        })
        .collect()
}

fn operator_access(policy: &str) -> HmiAccessConfig {
    HmiAccessConfig {
        users: vec![HmiUser {
            name: "anna".into(),
            role: AccessRole::Operator,
            password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
                .expect("hash"),
        }],
        write: hmi_access_from_runtime_toml(policy).expect("policy").write,
        ..HmiAccessConfig::default()
    }
}

#[test]
fn a_faceplate_placed_three_times_comes_with_the_page_bound_to_each_instance() {
    let root = placed_project("inline-three");
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        operator_access(
            "[[runtime.web.hmi.write]]\npath = \"Main.P101.StartCmd\"\nrole = \"engineer\"\n",
        ),
        PROGRAM,
        None,
    );
    let data = get(&running.base, "/api/hmi/page?name=overview");
    assert_eq!(data["errors"], json!([]), "{data}");
    let placed = &data["faceplates"];
    for (id, instance) in [
        ("card-1", "Main.P101"),
        ("card-2", "Main.P102"),
        ("card-3", "Main.P103"),
    ] {
        assert_eq!(
            placed[id]["name"],
            json!(format!("faceplate:overview/{id}")),
            "{data}"
        );
        assert_eq!(
            paths(&placed[id]["model"]),
            [
                format!("{instance}.ActualSpeed"),
                format!("{instance}.StartCmd")
            ],
            "{id}: bound as its popup is, Speed by the pair"
        );
    }
    assert!(
        placed.get("card-4").is_some_and(Value::is_null),
        "a faceplate the project lacks is named as such: {data}"
    );
    // The slim header names the instance: by the object's tag, else by the instance's own name.
    assert_eq!(placed["card-2"]["model"]["title"], json!("P-102"));
    assert_eq!(placed["card-1"]["model"]["title"], json!("P101"));
    assert_eq!(
        placed["card-1"]["model"]["description"],
        json!("Transfer pump")
    );
    // The role each control needs for its instance, as the popup says it (§5.9 item 5).
    assert_eq!(
        placed["card-1"]["roles"]["start"],
        json!("engineer"),
        "{data}"
    );
    assert!(
        placed["card-2"]["roles"]
            .get("start")
            .is_some_and(Value::is_null),
        "P102's command is not in the policy: {data}"
    );
    // One change to the faceplate file changes every placement (§8.9 item 3).
    std::fs::write(
        root.join("hmi/panels/pump.toml"),
        FACEPLATE.replace(
            "description = \"Transfer pump\"",
            "description = \"Feed pump\"",
        ),
    )
    .expect("write");
    let changed = get(&running.base, "/api/hmi/page?name=overview");
    for id in ["card-1", "card-2", "card-3"] {
        assert_eq!(
            changed["faceplates"][id]["model"]["description"],
            json!("Feed pump"),
            "{id}"
        );
    }
}

#[test]
fn the_pages_stream_carries_its_placed_faceplates_values() {
    let running = start_with(
        &placed_project("inline-stream"),
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    let address = running.base.trim_start_matches("http://").to_string();
    let stream = TcpStream::connect(&address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    let (mut socket, _) =
        tungstenite::client(format!("ws://{address}/ws/hmi?page=overview"), stream)
            .expect("the page's stream opens");
    let mut seen = false;
    for _ in 0..20 {
        let Ok(message) = socket.read() else {
            break;
        };
        let Message::Text(text) = message else {
            continue;
        };
        let message: Value = serde_json::from_str(text.as_str()).expect("JSON");
        if message["values"]["Main.P102.ActualSpeed"]["value"] == json!(1460.0) {
            seen = true;
            break;
        }
    }
    assert!(
        seen,
        "the page's stream carries the second placement's speed"
    );
}

#[test]
fn a_command_from_a_placed_faceplate_writes_its_instances_member() {
    let running = start_with(
        &placed_project("inline-command"),
        WebAuthMode::Local,
        None,
        operator_access(
            "[[runtime.web.hmi.write]]\npath = \"Main.P101.StartCmd\"\nrole = \"operator\"\n",
        ),
        PROGRAM,
        None,
    );
    let cookie = session_cookie(&running.base, "anna");
    let bundle = get(&running.base, "/api/hmi/page?name=overview")["bundle"].clone();
    let body = json!({ "id": "c1", "bundle": bundle, "page": "faceplate:overview/card-1", "object": "start", "action": "set" });
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
