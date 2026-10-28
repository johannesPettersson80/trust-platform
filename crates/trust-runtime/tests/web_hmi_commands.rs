//! Specification 39, §12.2 and §12.5–12.8: commands from the runtime HMI against a running
//! program: permission, limits, readback and confirmation, duplicates, out-of-date pages,
//! interlocks, pulses and the audit log.

use std::net::TcpStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::control::ControlAuditEvent;
use trust_runtime::hmi::access::config::{hmi_access_from_runtime_toml, HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::security::AccessRole;
use tungstenite::{Message, WebSocket};

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

const PROGRAM: &str = r#"
PROGRAM Main
VAR
    Setpoint : REAL := 50.0;
    Start : BOOL;
    Running : BOOL;
    Locked : BOOL;
    Permit : BOOL := FALSE;
    PermitText : STRING := 'Door open';
    Pulse : BOOL;
    Pulses : INT;
    Clamp : REAL;
    Tuning : REAL := 1.0;
    Spare : REAL;
END_VAR
    Running := Start;
    IF Pulse THEN
        Pulses := Pulses + 1;
    END_IF;
    Clamp := 10.0;
END_PROGRAM
"#;

const OPERATE: &str = r#"format = 1
title = "Operate"

[[object]]
id = "sp"
type = "setpoint"
at = [0, 0]
size = [200, 48]
min = 0
max = 100
timeout = 2

[object.bind]
value = { path = "Main.Setpoint", direction = "read-write" }

[[object]]
id = "start"
type = "button"
at = [0, 60]
size = [120, 40]
action = "set"
timeout = 2

[object.bind]
feedback = "Main.Running"
target = { path = "Main.Start", direction = "write" }

[[object]]
id = "locked"
type = "button"
at = [0, 120]
size = [120, 40]

[object.bind]
interlock = "Main.Permit"
interlock-reason = "Main.PermitText"
target = { path = "Main.Locked", direction = "write" }

[[object]]
id = "tuning"
type = "setpoint"
at = [0, 180]
size = [200, 48]
role = "engineer"

[object.bind]
value = { path = "Main.Tuning", direction = "read-write" }

[[object]]
id = "pulse"
type = "pulse-button"
at = [0, 240]
size = [120, 40]
pulse = 0.3

[object.bind]
target = { path = "Main.Pulse", direction = "write" }

[[object]]
id = "pulses"
type = "value"
at = [200, 240]
size = [160, 48]

[object.bind]
value = "Main.Pulses"

[[object]]
id = "pulse-state"
type = "status"
at = [380, 240]
size = [40, 40]

[object.bind]
state = "Main.Pulse"

[[object]]
id = "clamp"
type = "setpoint"
at = [0, 300]
size = [200, 48]
timeout = 0.5

[object.bind]
value = { path = "Main.Clamp", direction = "read-write" }

[[object]]
id = "spare"
type = "setpoint"
at = [0, 360]
size = [200, 48]

[object.bind]
value = { path = "Main.Spare", direction = "read-write" }
"#;

const POLICY: &str = r#"
[[runtime.web.hmi.write]]
path = "Main.Setpoint"
role = "operator"
min = 0.0
max = 100.0

[[runtime.web.hmi.write]]
path = "Main.Start"
role = "operator"

[[runtime.web.hmi.write]]
path = "Main.Locked"
role = "operator"

[[runtime.web.hmi.write]]
path = "Main.Tuning"
role = "operator"

[[runtime.web.hmi.write]]
path = "Main.Pulse"
role = "operator"

[[runtime.web.hmi.write]]
path = "Main.Clamp"
role = "operator"
"#;

struct Plant {
    running: Running,
    cookie: String,
    bundle: Value,
    audit: mpsc::Receiver<ControlAuditEvent>,
}

fn plant(name: &str) -> Plant {
    let root = project(
        name,
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/operate.toml", OPERATE),
        ],
    );
    let user = |name: &str, role| HmiUser {
        name: name.into(),
        role,
        password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
            .expect("hash"),
    };
    let access = HmiAccessConfig {
        users: vec![
            user("anna", AccessRole::Operator),
            user("erik", AccessRole::Engineer),
        ],
        write: hmi_access_from_runtime_toml(POLICY)
            .expect("the policy")
            .write,
        ..HmiAccessConfig::default()
    };
    let (audit_tx, audit) = mpsc::channel();
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        access,
        PROGRAM,
        Some(audit_tx),
    );
    let cookie = session_cookie(&running.base, "anna");
    let page = http_request(
        "GET",
        &running.base,
        "/api/hmi/page?name=operate",
        None,
        None,
    )
    .1;
    let bundle = page["bundle"].clone();
    assert!(
        bundle.is_string(),
        "the page data names its version: {page}"
    );
    Plant {
        running,
        cookie,
        bundle,
        audit,
    }
}

impl Plant {
    fn command(&self, id: &str, object: &str, action: &str, value: Option<Value>) -> Value {
        self.command_as(Some(&self.cookie), id, &self.bundle, object, action, value)
    }

    fn command_as(
        &self,
        cookie: Option<&str>,
        id: &str,
        bundle: &Value,
        object: &str,
        action: &str,
        value: Option<Value>,
    ) -> Value {
        let mut body = json!({ "id": id, "bundle": bundle, "page": "operate", "object": object, "action": action });
        if let Some(value) = value {
            body["value"] = value;
        }
        let (status, answer, _) = http_request(
            "POST",
            &self.running.base,
            "/api/hmi/command",
            cookie,
            Some(body),
        );
        assert_eq!(status, 200, "{answer}");
        assert_eq!(answer["id"], json!(id), "{answer}");
        answer
    }

    /// The value stream with the session, for later outcomes and values.
    fn stream(&self) -> WebSocket<TcpStream> {
        let address = self.running.base.trim_start_matches("http://").to_string();
        let stream = TcpStream::connect(&address).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_millis(300)))
            .expect("timeout");
        let request = tungstenite::http::Request::builder()
            .uri(format!("ws://{address}/ws/hmi?page=operate"))
            .header("Host", &address)
            .header("Cookie", &self.cookie)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tungstenite::handshake::client::generate_key(),
            )
            .body(())
            .expect("a request");
        tungstenite::client(request, stream)
            .expect("the stream opens")
            .0
    }
}

/// Stream messages until `found` gives an answer, or `seconds` pass.
fn watch<T>(
    socket: &mut WebSocket<TcpStream>,
    seconds: u64,
    mut found: impl FnMut(&Value) -> Option<T>,
) -> Option<T> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < deadline {
        if let Ok(Message::Text(text)) = socket.read() {
            let message: Value = serde_json::from_str(text.as_str()).expect("JSON");
            if let Some(answer) = found(&message) {
                return Some(answer);
            }
        }
    }
    None
}

fn later_outcome(socket: &mut WebSocket<TcpStream>, id: &str) -> Option<Value> {
    watch(socket, 4, |message| {
        let command = message.get("command")?;
        (command["id"] == json!(id) && command["outcome"] != json!("applied"))
            .then(|| command.clone())
    })
}

#[test]
fn an_operator_changes_a_permitted_setpoint_and_sees_it_confirmed_by_readback() {
    let plant = plant("setpoint");
    let mut socket = plant.stream();
    let answer = plant.command("c-setpoint", "sp", "write", Some(json!(61.5)));
    assert_eq!(
        (&answer["outcome"], &answer["readback"]),
        (&json!("applied"), &json!(61.5)),
        "{answer}"
    );
    let confirmed = later_outcome(&mut socket, "c-setpoint").expect("a later outcome");
    assert_eq!(
        (&confirmed["outcome"], &confirmed["readback"]),
        (&json!("confirmed"), &json!(61.5)),
        "{confirmed}"
    );
}

/// Specification 39, §12.8: a number entry is sent as the operator typed it and the runtime reads
/// it, a comma or a point before the decimals, so `61,5` writes 61.5; text that is no number is
/// refused with the reason and writes nothing. Catches a runtime that writes the text as it came,
/// takes `61,5` as 61 or 615, or leaves the check to the browser.
#[test]
fn a_typed_number_entry_is_read_by_the_runtime() {
    let plant = plant("setpoint-entry");
    let answer = plant.command("c-comma", "sp", "write", Some(json!("61,5")));
    assert_eq!(
        (&answer["outcome"], &answer["readback"]),
        (&json!("applied"), &json!(61.5)),
        "{answer}"
    );
    let refused = plant.command("c-word", "sp", "write", Some(json!("fast")));
    assert_eq!(refused["outcome"], json!("refused"), "{refused}");
    assert!(
        refused["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("enter a number, not \"fast\"")),
        "{refused}"
    );
}

/// Specification 39, §5.9 item 5: the page says which role each control needs (the policy's,
/// raised by the control's own), so the display dims what the signed-in role may not use; a control
/// the policy does not cover is listed without a role.
#[test]
fn the_page_says_which_role_each_control_needs() {
    let plant = plant("roles");
    let page = http_request(
        "GET",
        &plant.running.base,
        "/api/hmi/page?name=operate",
        None,
        None,
    )
    .1;
    let roles = &page["roles"];
    assert_eq!(roles["sp"], json!("operator"), "{page}");
    assert_eq!(
        roles["tuning"],
        json!("engineer"),
        "raised by the control: {roles}"
    );
    assert!(
        roles.get("spare").is_some_and(Value::is_null),
        "not in the policy: {roles}"
    );
    assert!(
        roles.get("pulses").is_none(),
        "a value is no control: {roles}"
    );
}

#[test]
fn a_denied_write_is_refused_with_its_reason() {
    let plant = plant("denied");
    let refused = |answer: Value, words: &str| {
        assert_eq!(answer["outcome"], json!("refused"), "{answer}");
        assert!(
            answer["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains(words)),
            "{words}: {answer}"
        );
    };
    refused(
        plant.command_as(None, "c-1", &plant.bundle, "sp", "write", Some(json!(10))),
        "log in",
    );
    refused(
        plant.command("c-2", "sp", "write", Some(json!(150))),
        "above",
    );
    refused(
        plant.command("c-3", "tuning", "write", Some(json!(2))),
        "Engineer",
    );
    refused(
        plant.command("c-4", "spare", "write", Some(json!(2))),
        "write policy",
    );
    refused(
        plant.command("c-5", "sp", "write", Some(json!("much"))),
        "number",
    );
    refused(
        plant.command("c-6", "pulses", "write", Some(json!(1))),
        "no command",
    );
    refused(plant.command("c-7", "start", "reset", None), "set");
    let engineer = session_cookie(&plant.running.base, "erik");
    let allowed = plant.command_as(
        Some(&engineer),
        "c-8",
        &plant.bundle,
        "tuning",
        "write",
        Some(json!(2)),
    );
    assert_eq!(allowed["outcome"], json!("applied"), "{allowed}");
}

#[test]
fn a_repeated_command_id_is_executed_at_most_once() {
    let plant = plant("duplicate");
    let first = plant.command("c-twice", "sp", "write", Some(json!(70)));
    let again = plant.command("c-twice", "sp", "write", Some(json!(20)));
    assert_eq!(again, first, "the first answer, without a second write");
    let readback = plant.command("c-read", "sp", "write", Some(json!(70)));
    assert_eq!(readback["readback"], json!(70.0));
}

#[test]
fn a_command_from_an_out_of_date_page_is_refused() {
    let plant = plant("stale");
    let answer = plant.command_as(
        Some(&plant.cookie),
        "c-old",
        &json!("an old version"),
        "sp",
        "write",
        Some(json!(10)),
    );
    assert_eq!(answer["outcome"], json!("refused"), "{answer}");
    assert!(
        answer["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("reload")),
        "{answer}"
    );
}

#[test]
fn a_button_is_confirmed_by_its_feedback() {
    let plant = plant("button");
    let mut socket = plant.stream();
    assert_eq!(
        plant.command("c-start", "start", "set", None)["outcome"],
        json!("applied")
    );
    let confirmed = later_outcome(&mut socket, "c-start").expect("a later outcome");
    assert_eq!(confirmed["outcome"], json!("confirmed"), "{confirmed}");
}

#[test]
fn an_interlocked_command_is_refused_with_the_interlocks_reason() {
    let plant = plant("interlock");
    let answer = plant.command("c-locked", "locked", "set", None);
    assert_eq!(answer["outcome"], json!("refused"), "{answer}");
    assert!(
        answer["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("Door open")),
        "{answer}"
    );
}

#[test]
fn a_pulse_ends_by_itself() {
    let plant = plant("pulse");
    let mut socket = plant.stream();
    assert_eq!(
        plant.command("c-pulse", "pulse", "pulse", None)["outcome"],
        json!("applied")
    );
    std::thread::sleep(Duration::from_millis(1200));
    // The newest value of each path, as the page keeps them: an update carries only what changed
    // (§18 phase 5 item 4), and messages queued while the test slept come first.
    let mut newest = serde_json::Map::new();
    let _ = watch(&mut socket, 1, |message| -> Option<()> {
        if let Some(Value::Object(latest)) = message.get("values") {
            for (path, value) in latest {
                newest.insert(path.clone(), value.clone());
            }
        }
        None
    });
    let values = Value::Object(newest);
    assert_eq!(
        values["Main.Pulse"]["value"],
        json!(false),
        "the pulse ended: {values}"
    );
    assert!(
        values["Main.Pulses"]["value"]
            .as_i64()
            .is_some_and(|scans| scans > 0),
        "it was on for some scans: {values}"
    );
}

#[test]
fn a_value_the_program_overrides_times_out_with_the_value_read_back() {
    let plant = plant("override");
    let mut socket = plant.stream();
    assert_eq!(
        plant.command("c-clamp", "clamp", "write", Some(json!(50)))["outcome"],
        json!("applied")
    );
    let outcome = later_outcome(&mut socket, "c-clamp").expect("a later outcome");
    assert_eq!(
        (&outcome["outcome"], &outcome["readback"]),
        (&json!("timed-out"), &json!(10.0)),
        "{outcome}"
    );
}

#[test]
fn every_command_appears_in_the_audit_log() {
    let plant = plant("audit");
    plant.command("c-audit", "sp", "write", Some(json!(42)));
    plant.command("c-refused", "sp", "write", Some(json!(500)));
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut records = Vec::new();
    while Instant::now() < deadline {
        if let Ok(event) = plant.audit.recv_timeout(Duration::from_millis(100))
            && event.request_type == "hmi.command"
        {
            records.push(event.details.unwrap_or(Value::Null));
        }
    }
    let applied = records
        .iter()
        .find(|record| record["command"] == json!("c-audit"))
        .expect("the applied command");
    assert_eq!(applied["user"], json!("anna"));
    assert!(applied["session"].as_str().is_some_and(|id| !id.is_empty()));
    assert_eq!(applied["target"], json!("Main.Setpoint"));
    assert_eq!(applied["old"], json!(50.0));
    assert_eq!(applied["requested"], json!(42));
    assert_eq!(applied["bundle"], plant.bundle);
    assert!(applied["time_ms"].as_u64().is_some());
    assert!(
        records
            .iter()
            .any(|record| record["command"] == json!("c-audit")
                && record["outcome"] == json!("confirmed")),
        "{records:?}"
    );
    let refused = records
        .iter()
        .find(|record| record["command"] == json!("c-refused"))
        .expect("the refused command");
    assert_eq!(refused["outcome"], json!("refused"));
    assert!(refused["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("above")));
}
