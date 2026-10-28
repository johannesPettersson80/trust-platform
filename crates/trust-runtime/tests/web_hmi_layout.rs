//! Specification 39, §4.6 and §4.9: the page list the runtime display navigates by, and popups
//! with their values and commands.

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

const ALARMS: &str = "format = 1\ntitle = \"Alarms\"\norder = 1\nicon = \"alarm\"\n";
const UTILITIES: &str = "format = 1\ntitle = \"Utilities\"\norder = 2\ngroup = \"Plant\"\n";
const SERVICE: &str = "format = 1\ntitle = \"Service\"\nhidden = true\n";
const OVERVIEW_FIRST: &str = "format = 1\ntitle = \"Overview\"\norder = 0\nicon = \"overview\"\n";

const PUMP_POPUP: &str = r#"format = 1
title = "Pump P101"
size = [400, 240]
pinnable = true

[[object]]
id = "speed"
type = "value"
at = [24, 24]
size = [200, 96]

[object.bind]
value = "Main.P101.Speed"

[[object]]
id = "start"
type = "button"
at = [24, 140]
size = [120, 44]
action = "set"

[object.bind]
target = { path = "Main.Start", direction = "write" }
"#;

const POPUP_PROGRAM: &str = r#"
FUNCTION_BLOCK Pump
VAR_INPUT
    StartCmd : BOOL;
END_VAR
VAR_OUTPUT
    Running : BOOL;
    Speed : REAL;
END_VAR
    Running := StartCmd;
    IF Running THEN
        Speed := 1460.0;
    END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    P101 : Pump;
    Start : BOOL;
END_VAR
    P101(StartCmd := TRUE);
END_PROGRAM
"#;

fn get(base: &str, path: &str) -> (u16, Value) {
    let (status, body, _) = http_request("GET", base, path, None, None);
    (status, body)
}

#[test]
fn the_page_data_lists_the_pages_in_their_order_without_the_hidden_ones() {
    let root = project(
        "layout-list",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/zeta.toml", "format = 1\ntitle = \"Zeta\"\n"),
            ("hmi/pages/alarms.toml", ALARMS),
            ("hmi/pages/utilities.toml", UTILITIES),
            ("hmi/pages/service.toml", SERVICE),
            ("hmi/pages/overview.toml", OVERVIEW_FIRST),
        ],
    );
    let running = start(&root, WebAuthMode::Local, None);
    let (status, data) = get(&running.base, "/api/hmi/page");
    assert_eq!(status, 200, "{data}");
    assert_eq!(
        data["navigation"],
        json!([
            { "name": "overview", "title": "Overview", "icon": "overview" },
            { "name": "alarms", "title": "Alarms", "icon": "alarm" },
            { "name": "utilities", "title": "Utilities", "group": "Plant" },
            { "name": "zeta", "title": "Zeta" },
        ])
    );
    assert_eq!(
        data["page"]["name"],
        json!("overview"),
        "/hmi opens the first page of the list"
    );
    let (_, hidden) = get(&running.base, "/api/hmi/page?name=service");
    assert_eq!(
        hidden["page"]["model"]["title"],
        json!("Service"),
        "a hidden page is still reached by name"
    );
}

#[test]
fn a_popup_is_served_with_its_size_and_its_values_stream() {
    let root = project(
        "layout-popup",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", OVERVIEW),
            ("hmi/popups/pump.toml", PUMP_POPUP),
        ],
    );
    let running = start(&root, WebAuthMode::Local, None);
    let (status, data) = get(&running.base, "/api/hmi/popup?name=pump");
    assert_eq!(status, 200, "{data}");
    assert_eq!(data["popup"]["name"], json!("pump"));
    assert_eq!(data["popup"]["model"]["size"], json!([400, 240]));
    assert_eq!(data["popup"]["model"]["pinnable"], json!(true));
    assert_eq!(data["popup"]["model"]["title"], json!("Pump P101"));
    assert!(data["bundle"].is_string(), "{data}");
    assert_eq!(get(&running.base, "/api/hmi/popup?name=none").0, 404);

    let address = running.base.trim_start_matches("http://").to_string();
    let stream = TcpStream::connect(&address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("timeout");
    let (mut socket, _) = tungstenite::client(format!("ws://{address}/ws/hmi?popup=pump"), stream)
        .expect("the popup's stream opens");
    let speed = loop {
        let Message::Text(text) = socket.read().expect("a message") else {
            continue;
        };
        let message: Value = serde_json::from_str(text.as_str()).expect("JSON");
        if message["values"]["Main.P101.Speed"]["value"] == json!(1460.0) {
            break message;
        }
    };
    assert!(
        speed["values"].get("Main.P101.Running").is_none(),
        "only the popup's paths: {speed}"
    );
}

#[test]
fn a_command_from_a_popup_names_it_as_its_page() {
    let root = project(
        "layout-popup-command",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", OVERVIEW),
            ("hmi/popups/pump.toml", PUMP_POPUP),
        ],
    );
    let access = HmiAccessConfig {
        users: vec![HmiUser {
            name: "anna".into(),
            role: AccessRole::Operator,
            password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
                .expect("hash"),
        }],
        write: hmi_access_from_runtime_toml(
            "[[runtime.web.hmi.write]]\npath = \"Main.Start\"\nrole = \"operator\"\n",
        )
        .expect("policy")
        .write,
        ..HmiAccessConfig::default()
    };
    let running = start_with(&root, WebAuthMode::Local, None, access, POPUP_PROGRAM, None);
    let cookie = session_cookie(&running.base, "anna");
    let bundle = get(&running.base, "/api/hmi/popup?name=pump").1["bundle"].clone();
    let body = json!({ "id": "p1", "bundle": bundle, "page": "popup:pump", "object": "start", "action": "set" });
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
fn project_images_are_served_from_the_active_bundles_assets_and_nothing_outside_them() {
    let root = project(
        "layout-assets",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", OVERVIEW),
            (
                "hmi/assets/pumps/p101.svg",
                "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
            ),
            ("runtime.toml", "secret"),
        ],
    );
    let running = start(&root, WebAuthMode::Local, None);
    let url = format!("{}/hmi/project-assets/pumps/p101.svg", running.base);
    let mut response = ureq::get(&url).call().expect("the image");
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response
            .headers()
            .get("Content-Type")
            .and_then(|value| value.to_str().ok()),
        Some("image/svg+xml")
    );
    assert!(response
        .body_mut()
        .read_to_string()
        .expect("text")
        .contains("<svg"));
    for outside in [
        "/hmi/project-assets/../../runtime.toml",
        "/hmi/project-assets/%2e%2e/runtime.toml",
        "/hmi/project-assets/missing.png",
    ] {
        let status = ureq::get(&format!("{}{outside}", running.base))
            .config()
            .http_status_as_error(false)
            .build()
            .call()
            .map(|response| response.status().as_u16())
            .unwrap_or(0);
        assert_eq!(status, 404, "{outside}");
    }
}
