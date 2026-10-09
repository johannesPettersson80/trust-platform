//! Specification 39, §21.2 and §21.5 item 2, Plant library §3.5–§3.6 and §12.2: commands that call
//! a method of the real Plant library's pump through its supplied panel. The runtime checks the
//! session's role, the interlock and the inputs, calls the method at a display boundary, takes its
//! BOOL result as the outcome and names the pump's `CommandText` when it refuses. Replay records
//! are made only for authorized commands and are counted per user. No write rule is needed.

use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::hmi::access::config::{HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::security::AccessRole;
use tungstenite::{Message, WebSocket};

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

/// The Alarm and Control libraries, then every source file of the Plant library.
const SOURCES: &[&str] = &[
    include_str!("../../../libraries/alarm/src/alarm.st"),
    include_str!("../../../libraries/control/src/control.st"),
    include_str!("../../../libraries/plant/src/types.st"),
    include_str!("../../../libraries/plant/src/functions.st"),
    include_str!("../../../libraries/plant/src/interfaces.st"),
    include_str!("../../../libraries/plant/src/device.st"),
    include_str!("../../../libraries/plant/src/end_position.st"),
    include_str!("../../../libraries/plant/src/motor.st"),
    include_str!("../../../libraries/plant/src/drive.st"),
    include_str!("../../../libraries/plant/src/heater.st"),
    include_str!("../../../libraries/plant/src/two_output_motor.st"),
    include_str!("../../../libraries/plant/src/valve.st"),
    include_str!("../../../libraries/plant/src/motor_valve.st"),
    include_str!("../../../libraries/plant/src/cylinder.st"),
    include_str!("../../../libraries/plant/src/control_valve.st"),
    include_str!("../../../libraries/plant/src/totals.st"),
    include_str!("../../../libraries/plant/src/analog_input.st"),
    include_str!("../../../libraries/plant/src/digital_input.st"),
    include_str!("../../../libraries/plant/src/scale_and_tank.st"),
    include_str!("../../../libraries/plant/src/controllers.st"),
    include_str!("../../../libraries/plant/src/conditions.st"),
    include_str!("../../../libraries/plant/src/groups.st"),
    include_str!("../../../libraries/plant/src/managers.st"),
    include_str!("../../../libraries/plant/src/sequence.st"),
];

/// The supplied panel of the motor family and the pump's component definition.
const RUN_PANEL: &str = include_str!("../../../libraries/plant/hmi/panels/plant-run.toml");
const PUMP_PAIR: &str = include_str!("../../../libraries/plant/hmi/pairs/plant-pump.toml");

const PROGRAM: &str = r#"
PROGRAM Main
VAR
    P101 : Plant.FB_Pump := (HasFeedback := FALSE);
END_VAR
    P101();
END_PROGRAM
"#;

/// The pump on a page, opening its panel, and a Start button of the page's own without an
/// interlock, so the pump's own answer is seen.
const OVERVIEW: &str = r#"format = 1
title = "Overview"

[[object]]
id = "pump"
tag = "P101"
type = "pump"
block = "Plant.FB_Pump"
instance = "Main.P101"
faceplate = "plant-run"
at = [40, 40]
size = [128, 108]

[object.bind]
running = "Main.P101.Running"

[[object]]
id = "start-direct"
type = "button"
text = "Start"
at = [240, 40]
size = [120, 48]

[object.command]
interface = "Plant.IRunDevice"
method = "Start"
instance = "Main.P101"
"#;

/// How commands and the stream name the pump's panel.
const PANEL: &str = "faceplate:overview/pump";

struct Plant {
    running: Running,
    bundle: Value,
}

fn user(name: &str, role: AccessRole) -> HmiUser {
    HmiUser {
        name: name.into(),
        role,
        password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
            .expect("hash"),
    }
}

/// The pump's project with operators anna and otto and engineer erik, and no write policy.
fn plant(name: &str) -> Plant {
    let root = project(
        name,
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", OVERVIEW),
            ("hmi/panels/plant-run.toml", RUN_PANEL),
            ("hmi/pairs/plant-pump.toml", PUMP_PAIR),
        ],
    );
    let access = HmiAccessConfig {
        users: vec![
            user("anna", AccessRole::Operator),
            user("erik", AccessRole::Engineer),
            user("otto", AccessRole::Operator),
        ],
        ..HmiAccessConfig::default()
    };
    let mut programs = SOURCES.to_vec();
    programs.push(PROGRAM);
    let running = start_with_sources(&root, WebAuthMode::Local, None, access, &programs, None);
    let (status, data, _) = http_request(
        "GET",
        &running.base,
        "/api/hmi/popup?faceplate=overview/pump",
        None,
        None,
    );
    assert_eq!(status, 200, "{data}");
    let bundle = data["bundle"].clone();
    assert!(bundle.is_string(), "{data}");
    Plant { running, bundle }
}

impl Plant {
    fn login(&self, user: &str) -> String {
        session_cookie(&self.running.base, user)
    }

    /// Sends `invoke` for an object of a page or of the pump's panel.
    fn invoke(
        &self,
        cookie: &str,
        id: &str,
        page: &str,
        object: &str,
        value: Option<Value>,
    ) -> Value {
        let mut body = json!({
            "id": id,
            "bundle": self.bundle,
            "page": page,
            "object": object,
            "action": "invoke",
        });
        if let Some(value) = value {
            body["value"] = value;
        }
        let (status, answer, _) = http_request(
            "POST",
            &self.running.base,
            "/api/hmi/command",
            Some(cookie),
            Some(body),
        );
        assert_eq!(status, 200, "{answer}");
        assert_eq!(answer["id"], json!(id), "{answer}");
        answer
    }

    fn panel(&self, cookie: &str, id: &str, object: &str, value: Option<Value>) -> Value {
        self.invoke(cookie, id, PANEL, object, value)
    }

    fn values(&self) -> Values {
        Values::open(&self.running.base)
    }
}

/// The panel's value stream: the newest value of each of the pump's members it shows.
struct Values {
    socket: WebSocket<TcpStream>,
    newest: Map<String, Value>,
}

impl Values {
    fn open(base: &str) -> Self {
        let address = base.trim_start_matches("http://").to_string();
        let stream = TcpStream::connect(&address).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("timeout");
        let (socket, _) = tungstenite::client(
            format!("ws://{address}/ws/hmi?faceplate=overview/pump"),
            stream,
        )
        .expect("the panel's stream opens");
        Self {
            socket,
            newest: Map::new(),
        }
    }

    fn read(&mut self) {
        if let Ok(Message::Text(text)) = self.socket.read() {
            let message: Value = serde_json::from_str(text.as_str()).expect("JSON");
            if let Some(Value::Object(values)) = message.get("values") {
                for (path, value) in values {
                    self.newest.insert(path.clone(), value["value"].clone());
                }
            }
        }
    }

    /// Whether `path` reads `expected` within `seconds`.
    fn reaches(&mut self, path: &str, expected: Value, seconds: u64) -> bool {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            if self.newest.get(path) == Some(&expected) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            self.read();
        }
    }

    /// The newest value of `path` after following the stream for `millis`.
    fn after(&mut self, path: &str, millis: u64) -> Value {
        let deadline = Instant::now() + Duration::from_millis(millis);
        while Instant::now() < deadline {
            self.read();
        }
        self.newest.get(path).cloned().unwrap_or(Value::Null)
    }
}

fn outcome(answer: &Value) -> &Value {
    &answer["outcome"]
}

#[test]
fn start_is_accepted_in_manual_and_the_pump_runs() {
    let plant = plant("method-start");
    let mut values = plant.values();
    let anna = plant.login("anna");
    let manual = plant.panel(&anna, "c-manual", "mode", Some(json!("Manual")));
    assert_eq!(outcome(&manual), &json!("applied"), "{manual}");
    assert!(
        values.reaches("Main.P101.StartPermit", json!(true), 3),
        "Manual and nothing blocks it: {:?}",
        values.newest
    );
    let start = plant.panel(&anna, "c-start", "start", None);
    assert_eq!(outcome(&start), &json!("applied"), "{start}");
    assert!(
        values.reaches("Main.P101.Running", json!(true), 3),
        "the pump runs: {:?}",
        values.newest
    );
}

#[test]
fn start_is_refused_in_auto_with_the_pumps_command_text() {
    let plant = plant("method-auto");
    let mut values = plant.values();
    let anna = plant.login("anna");
    assert!(values.reaches("Main.P101.CommandText", json!("Auto: switch to Manual"), 3));
    // The page's own button has no interlock: the pump itself answers FALSE.
    let refused = plant.invoke(&anna, "c-auto", "overview", "start-direct", None);
    assert_eq!(
        (outcome(&refused), &refused["reason"]),
        (&json!("refused"), &json!("Auto: switch to Manual")),
        "{refused}"
    );
    assert_eq!(
        values.after("Main.P101.Running", 300),
        json!(false),
        "a refused start leaves nothing pending (Plant §3.5)"
    );
}

#[test]
fn an_interlock_permit_that_is_false_refuses_with_its_reason() {
    let plant = plant("method-interlock");
    let mut values = plant.values();
    let anna = plant.login("anna");
    assert!(values.reaches("Main.P101.StartPermit", json!(false), 3));
    let refused = plant.panel(&anna, "c-interlocked", "start", None);
    assert_eq!(outcome(&refused), &json!("refused"), "{refused}");
    assert_eq!(
        refused["reason"],
        json!("Auto: switch to Manual"),
        "the panel's Start permit and its reason, the pump's CommandText"
    );
}

#[test]
fn an_engineer_button_is_refused_for_an_operator() {
    let plant = plant("method-role");
    let mut values = plant.values();
    let anna = plant.login("anna");
    let refused = plant.panel(&anna, "c-oos-anna", "out-of-service", None);
    assert_eq!(outcome(&refused), &json!("refused"), "{refused}");
    assert!(
        refused["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("Engineer")),
        "{refused}"
    );
    let erik = plant.login("erik");
    let applied = plant.panel(&erik, "c-oos-erik", "out-of-service", None);
    assert_eq!(outcome(&applied), &json!("applied"), "{applied}");
    assert!(values.reaches("Main.P101.Mode", json!("OutOfService"), 3));
}

#[test]
fn a_mode_the_selector_does_not_offer_is_refused() {
    let plant = plant("method-options");
    let mut values = plant.values();
    let anna = plant.login("anna");
    let refused = plant.panel(&anna, "c-oos", "mode", Some(json!("OutOfService")));
    assert_eq!(outcome(&refused), &json!("refused"), "{refused}");
    assert!(
        refused["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("not one of its modes")),
        "{refused}"
    );
    assert_eq!(values.after("Main.P101.Mode", 300), json!("Auto"));
}

#[test]
fn a_repeated_command_id_calls_the_method_once() {
    let plant = plant("method-replay");
    let mut values = plant.values();
    let anna = plant.login("anna");
    assert_eq!(
        outcome(&plant.panel(&anna, "c-manual", "mode", Some(json!("Manual")))),
        &json!("applied")
    );
    assert!(values.reaches("Main.P101.StartPermit", json!(true), 3));
    let first = plant.panel(&anna, "c-start", "start", None);
    assert_eq!(outcome(&first), &json!("applied"), "{first}");
    assert!(values.reaches("Main.P101.Running", json!(true), 3));
    assert_eq!(
        outcome(&plant.panel(&anna, "c-stop", "stop", None)),
        &json!("applied")
    );
    assert!(values.reaches("Main.P101.Running", json!(false), 3));
    let again = plant.panel(&anna, "c-start", "start", None);
    assert_eq!(again, first, "the first answer");
    assert_eq!(
        values.after("Main.P101.Running", 400),
        json!(false),
        "Start was not called a second time"
    );
    assert_eq!(values.after("Main.P101.Starts", 0), json!(1));
}

#[test]
fn one_users_records_never_take_another_users_room() {
    let plant = plant("method-per-user");
    let anna = plant.login("anna");
    // Choosing Auto again is accepted every time: each answered command keeps a replay record.
    // (Reset would be refused before any record: it is permitted only while the pump has a fault.)
    let auto = || Some(json!("Auto"));
    for index in 0..64 {
        let answer = plant.panel(&anna, &format!("mode-{index}"), "mode", auto());
        assert_eq!(outcome(&answer), &json!("applied"), "{answer}");
    }
    let otto = plant.login("otto");
    let other = plant.panel(&otto, "mode-otto", "mode", auto());
    assert_eq!(
        outcome(&other),
        &json!("applied"),
        "another user's command: {other}"
    );
    let next = plant.panel(&anna, "mode-64", "mode", auto());
    assert_eq!(
        outcome(&next),
        &json!("applied"),
        "anna's oldest answered record makes room: {next}"
    );
}
