//! Specification 40, §4 and §5: the runtime's alarm service over the web: the alarm list and the
//! records, operator actions with their roles and confirmation, the value stream and the audit
//! log, against a running program that uses the `Alarm` library.

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

const ALARM: &str = include_str!("../../../libraries/alarm/src/alarm.st");

const PROGRAM: &str = r#"
USING Alarm;

PROGRAM Tank
VAR
    Level : LREAL := 95.0;
    Trip : BOOL;
    High : LimitAlarm;
    Overflow : DigitalAlarm;
END_VAR
    High(Value := Level, Limit := 90.0, Message := 'Tank level high', Priority := AlarmPriority#Medium);
    Overflow(Condition := Trip, Message := 'Tank overflow', Priority := AlarmPriority#High, Latching := TRUE);
END_PROGRAM
"#;

const PLANT_PAGE: &str = r#"format = 1
title = "Plant"

[[object]]
id = "trip"
type = "switch"
at = [0, 0]
size = [88, 48]

[object.bind]
value = { path = "Tank.Trip", direction = "read-write" }
"#;

const POLICY: &str = r#"
[[runtime.web.hmi.write]]
path = "Tank.Trip"
role = "operator"
"#;

struct Plant {
    running: Running,
    anna: String,
    erik: String,
    bundle: Value,
    audit: mpsc::Receiver<ControlAuditEvent>,
}

fn plant(name: &str) -> Plant {
    let root = project(
        name,
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/plant.toml", PLANT_PAGE),
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
    let source = format!("{ALARM}\n{PROGRAM}");
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        access,
        &source,
        Some(audit_tx),
    );
    let anna = session_cookie(&running.base, "anna");
    let erik = session_cookie(&running.base, "erik");
    let bundle = http_request("GET", &running.base, "/api/hmi/page?name=plant", None, None).1
        ["bundle"]
        .clone();
    let plant = Plant {
        running,
        anna,
        erik,
        bundle,
        audit,
    };
    // The service's first read lists the alarm that is active from the start.
    plant.wait_for("the first read", |alarms| {
        has(&alarms["alarms"]["active"], "Tank.High")
    });
    plant
}

impl Plant {
    fn alarms(&self) -> Value {
        let (status, body, _) =
            http_request("GET", &self.running.base, "/api/hmi/alarms", None, None);
        assert_eq!(status, 200, "{body}");
        body
    }

    /// The alarms once `found` holds, within three seconds.
    fn wait_for(&self, what: &str, found: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let (status, body, _) =
                http_request("GET", &self.running.base, "/api/hmi/alarms", None, None);
            if status == 200 && found(&body) {
                return body;
            }
            assert!(Instant::now() < deadline, "{what}: {status} {body}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn act(
        &self,
        cookie: Option<&str>,
        id: &str,
        alarm: &str,
        action: &str,
        value: Option<Value>,
    ) -> Value {
        let mut body = json!({ "id": id, "alarm": alarm, "action": action });
        if let Some(value) = value {
            body["value"] = value;
        }
        let (status, answer, _) = http_request(
            "POST",
            &self.running.base,
            "/api/hmi/alarm",
            cookie,
            Some(body),
        );
        assert_eq!(status, 200, "{answer}");
        assert_eq!(answer["id"], json!(id), "{answer}");
        answer
    }

    fn trip(&self, id: &str, on: bool) {
        let body = json!({ "id": id, "bundle": self.bundle, "page": "plant", "object": "trip", "action": "write", "value": on });
        let (status, answer, _) = http_request(
            "POST",
            &self.running.base,
            "/api/hmi/command",
            Some(&self.anna),
            Some(body),
        );
        assert_eq!(
            (status, &answer["outcome"]),
            (200, &json!("applied")),
            "{answer}"
        );
    }

    /// The value stream of the plant page with Anna's session.
    fn stream(&self) -> WebSocket<TcpStream> {
        let address = self.running.base.trim_start_matches("http://").to_string();
        let stream = TcpStream::connect(&address).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_millis(300)))
            .expect("timeout");
        let request = tungstenite::http::Request::builder()
            .uri(format!("ws://{address}/ws/hmi?page=plant"))
            .header("Host", &address)
            .header("Cookie", &self.anna)
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

/// Whether a list of alarms has the alarm at `path`.
fn has(list: &Value, path: &str) -> bool {
    list.as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["path"] == json!(path)))
}

fn row<'a>(list: &'a Value, path: &str) -> &'a Value {
    list.as_array()
        .and_then(|rows| rows.iter().find(|row| row["path"] == json!(path)))
        .unwrap_or_else(|| panic!("{path} is not in {list}"))
}

/// The records of `kind` for the alarm at `path`.
fn records<'a>(alarms: &'a Value, path: &str, kind: &str) -> Vec<&'a Value> {
    alarms["events"]
        .as_array()
        .map(|events| {
            events
                .iter()
                .filter(|event| event["path"] == json!(path) && event["kind"] == json!(kind))
                .collect()
        })
        .unwrap_or_default()
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
    watch(socket, 6, |message| {
        let command = message.get("command")?;
        (command["id"] == json!(id) && command["outcome"] != json!("applied"))
            .then(|| command.clone())
    })
}

fn refused(answer: &Value, words: &str) {
    assert_eq!(answer["outcome"], json!("refused"), "{answer}");
    let reason = answer["reason"].as_str().unwrap_or_default();
    assert!(reason.contains(words), "{words}: {answer}");
}

#[test]
fn the_alarm_list_shows_every_alarm_of_the_program_unacknowledged_first_then_by_priority() {
    let plant = plant("alarms-list");
    let alarms = plant.alarms();
    let high = row(&alarms["alarms"]["active"], "Tank.High");
    assert_eq!(high["message"], json!("Tank level high"));
    assert_eq!(high["priority"], json!("medium"));
    assert_eq!(high["state"], json!("ActiveUnacknowledged"));
    assert_eq!(
        (&high["active"], &high["unacknowledged"]),
        (&json!(true), &json!(true))
    );
    assert!(high["since_ms"].as_u64().is_some(), "{high}");
    plant.trip("t1", true);
    let alarms = plant.wait_for("the overflow", |alarms| {
        has(&alarms["alarms"]["active"], "Tank.Overflow")
    });
    let order: Vec<&Value> = alarms["alarms"]["active"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|row| &row["path"])
        .collect();
    assert_eq!(
        order,
        vec![&json!("Tank.Overflow"), &json!("Tank.High")],
        "the High one first"
    );
    assert_eq!(alarms["alarms"]["unacknowledged"], json!(2));
    assert_eq!(alarms["alarms"]["annunciating"], json!(2));
    assert_eq!(alarms["alarms"]["newest"]["path"], json!("Tank.Overflow"));
    let raised = records(&alarms, "Tank.Overflow", "raised");
    assert_eq!(raised.len(), 1, "{alarms}");
    assert_eq!(raised[0]["message"], json!("Tank overflow"));
    assert_eq!(raised[0]["priority"], json!("high"));
    assert!(raised[0]["time_ms"].as_u64().is_some());
    assert_eq!(raised[0]["user"], Value::Null, "the program raised it");
}

#[test]
fn an_operator_acknowledges_an_alarm_it_is_confirmed_and_the_record_names_them() {
    let plant = plant("alarms-acknowledge");
    let mut stream = plant.stream();
    let answer = plant.act(Some(&plant.anna), "a1", "Tank.High", "acknowledge", None);
    assert_eq!(answer["outcome"], json!("applied"), "{answer}");
    let later = later_outcome(&mut stream, "a1").expect("a later outcome");
    assert_eq!(later["outcome"], json!("confirmed"), "{later}");
    let alarms = plant.wait_for("the acknowledgement", |alarms| {
        !records(alarms, "Tank.High", "acknowledged").is_empty()
    });
    let high = row(&alarms["alarms"]["active"], "Tank.High");
    assert_eq!(high["state"], json!("ActiveAcknowledged"));
    assert_eq!(high["unacknowledged"], json!(false));
    let acknowledged = records(&alarms, "Tank.High", "acknowledged");
    assert_eq!(acknowledged.len(), 1, "{alarms}");
    assert_eq!(acknowledged[0]["user"], json!("anna"));
}

#[test]
fn taking_an_alarm_out_of_service_and_back_needs_an_engineer() {
    let plant = plant("alarms-service");
    refused(
        &plant.act(
            Some(&plant.anna),
            "o1",
            "Tank.High",
            "remove-from-service",
            None,
        ),
        "needs the Engineer role",
    );
    let answer = plant.act(
        Some(&plant.erik),
        "o2",
        "Tank.High",
        "remove-from-service",
        None,
    );
    assert_eq!(answer["outcome"], json!("applied"), "{answer}");
    let alarms = plant.wait_for("out of service", |alarms| {
        has(&alarms["alarms"]["out_of_service"], "Tank.High")
    });
    assert!(!has(&alarms["alarms"]["active"], "Tank.High"), "{alarms}");
    assert_eq!(
        records(&alarms, "Tank.High", "out-of-service")[0]["user"],
        json!("erik")
    );
    plant.act(
        Some(&plant.erik),
        "o3",
        "Tank.High",
        "return-to-service",
        None,
    );
    let alarms = plant.wait_for("back in service", |alarms| {
        has(&alarms["alarms"]["active"], "Tank.High")
    });
    assert_eq!(
        records(&alarms, "Tank.High", "in-service")[0]["user"],
        json!("erik")
    );
}

#[test]
fn an_action_needs_a_session_an_alarm_of_the_program_and_a_fitting_value() {
    let plant = plant("alarms-refused");
    refused(
        &plant.act(None, "r1", "Tank.High", "acknowledge", None),
        "log in to send commands",
    );
    refused(
        &plant.act(Some(&plant.anna), "r2", "Tank.Level", "acknowledge", None),
        "the program has no alarm \"Tank.Level\"",
    );
    refused(
        &plant.act(
            Some(&plant.anna),
            "r3",
            "Tank.High",
            "shelve",
            Some(json!(0)),
        ),
        "1 to 86400 seconds",
    );
    refused(
        &plant.act(Some(&plant.anna), "r4", "Tank.High", "silence", None),
        "acknowledge, shelve",
    );
    let first = plant.act(Some(&plant.anna), "r5", "Tank.High", "acknowledge", None);
    let again = plant.act(Some(&plant.anna), "r5", "Tank.High", "acknowledge", None);
    assert_eq!(first, again, "a repeated id gets the first answer");
}

#[test]
fn shelving_and_comments_are_recorded_with_the_user_and_their_detail() {
    let plant = plant("alarms-shelve");
    let mut stream = plant.stream();
    plant.act(
        Some(&plant.anna),
        "s1",
        "Tank.High",
        "shelve",
        Some(json!(600)),
    );
    let alarms = plant.wait_for("shelved", |alarms| {
        has(&alarms["alarms"]["shelved"], "Tank.High")
    });
    assert!(!has(&alarms["alarms"]["active"], "Tank.High"), "{alarms}");
    let shelved = records(&alarms, "Tank.High", "shelved");
    assert_eq!(
        (&shelved[0]["user"], &shelved[0]["detail"]),
        (&json!("anna"), &json!("10 min"))
    );
    let answer = plant.act(
        Some(&plant.anna),
        "s2",
        "Tank.High",
        "comment",
        Some(json!("Level gauge checked on site")),
    );
    assert_eq!(answer["outcome"], json!("applied"), "{answer}");
    let later = later_outcome(&mut stream, "s2").expect("a later outcome");
    assert_eq!(
        later["outcome"],
        json!("confirmed"),
        "the instance took it: {later}"
    );
    let comment = records(&plant.alarms(), "Tank.High", "comment")
        .first()
        .map(|record| (*record).clone())
        .expect("the comment's record");
    assert_eq!(comment["user"], json!("anna"));
    assert_eq!(comment["detail"], json!("Level gauge checked on site"));
    plant.act(Some(&plant.anna), "s3", "Tank.High", "unshelve", None);
    plant.wait_for("unshelved", |alarms| {
        has(&alarms["alarms"]["active"], "Tank.High")
    });
}

#[test]
fn a_latched_alarm_stays_active_until_it_is_reset_after_its_condition_is_gone() {
    let plant = plant("alarms-reset");
    let mut stream = plant.stream();
    plant.trip("t1", true);
    plant.wait_for("the overflow", |alarms| {
        has(&alarms["alarms"]["active"], "Tank.Overflow")
    });
    plant.trip("t2", false);
    std::thread::sleep(Duration::from_millis(300));
    let overflow = row(&plant.alarms()["alarms"]["active"], "Tank.Overflow").clone();
    assert_eq!(overflow["active"], json!(true), "latched: {overflow}");
    assert_eq!(
        overflow["latching"],
        json!(true),
        "the list says it latches: {overflow}"
    );
    let high = row(&plant.alarms()["alarms"]["active"], "Tank.High").clone();
    assert_eq!(high["latching"], json!(false), "{high}");
    plant.act(Some(&plant.anna), "x1", "Tank.Overflow", "reset", None);
    let later = later_outcome(&mut stream, "x1").expect("a later outcome");
    assert_eq!(later["outcome"], json!("confirmed"), "{later}");
    let alarms = plant.wait_for("the reset", |alarms| {
        !records(alarms, "Tank.Overflow", "reset").is_empty()
    });
    let overflow = row(&alarms["alarms"]["active"], "Tank.Overflow");
    assert_eq!(
        overflow["state"],
        json!("ClearedUnacknowledged"),
        "listed until acknowledged"
    );
    assert_eq!(
        records(&alarms, "Tank.Overflow", "reset")[0]["user"],
        json!("anna")
    );
}

#[test]
fn the_value_stream_sends_the_alarm_list_and_the_new_records() {
    let plant = plant("alarms-stream");
    let mut stream = plant.stream();
    let list = watch(&mut stream, 3, |message| {
        message
            .get("alarms")
            .filter(|alarms| has(&alarms["active"], "Tank.High"))
            .cloned()
    })
    .expect("the alarm list");
    assert_eq!(list["unacknowledged"], json!(1), "{list}");
    plant.trip("t1", true);
    // The changed list and the new record, in either order.
    let (mut raised, mut changed) = (None, false);
    watch(&mut stream, 3, |message| {
        if let Some(events) = message.get("events").and_then(Value::as_array) {
            raised = raised.take().or_else(|| {
                events
                    .iter()
                    .find(|event| {
                        event["path"] == json!("Tank.Overflow") && event["kind"] == json!("raised")
                    })
                    .cloned()
            });
        }
        changed |= message
            .get("alarms")
            .is_some_and(|alarms| alarms["unacknowledged"] == json!(2));
        (raised.is_some() && changed).then_some(())
    })
    .expect("the changed list and the record on the stream");
    assert_eq!(
        raised.expect("the record")["message"],
        json!("Tank overflow")
    );
}

#[test]
fn every_alarm_action_appears_in_the_audit_log() {
    let plant = plant("alarms-audit");
    plant.act(
        Some(&plant.anna),
        "audit-1",
        "Tank.High",
        "acknowledge",
        None,
    );
    plant.act(
        Some(&plant.anna),
        "audit-2",
        "Tank.High",
        "remove-from-service",
        None,
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut records = Vec::new();
    while Instant::now() < deadline {
        if let Ok(event) = plant.audit.recv_timeout(Duration::from_millis(100))
            && event.request_type == "hmi.alarm"
        {
            records.push(event.details.unwrap_or(Value::Null));
        }
    }
    let applied = records
        .iter()
        .find(|record| record["command"] == json!("audit-1"))
        .expect("the acknowledgement");
    assert_eq!(applied["user"], json!("anna"));
    assert!(applied["session"].as_str().is_some_and(|id| !id.is_empty()));
    assert_eq!(applied["alarm"], json!("Tank.High"));
    assert_eq!(applied["action"], json!("acknowledge"));
    assert_eq!(applied["outcome"], json!("applied"));
    assert!(applied["time_ms"].as_u64().is_some());
    assert!(
        records
            .iter()
            .any(|record| record["command"] == json!("audit-1")
                && record["outcome"] == json!("confirmed")),
        "{records:?}"
    );
    let refused = records
        .iter()
        .find(|record| record["command"] == json!("audit-2"))
        .expect("the refused action");
    assert_eq!(refused["outcome"], json!("refused"));
    assert!(refused["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("Engineer")));
}

#[test]
fn each_commands_final_outcome_is_a_record_beside_the_alarms() {
    let plant = plant("alarms-command-records");
    plant.trip("t1", true);
    plant.act(Some(&plant.anna), "c1", "Tank.High", "acknowledge", None);
    plant.act(
        Some(&plant.anna),
        "c2",
        "Tank.High",
        "remove-from-service",
        None,
    );
    let alarms = plant.wait_for("the final outcomes", |alarms| {
        let commands = records(alarms, "Tank.High", "command");
        commands
            .iter()
            .any(|record| record["outcome"] == json!("confirmed"))
            && !records(alarms, "Tank.Trip", "command").is_empty()
    });
    let acknowledged = records(&alarms, "Tank.High", "command");
    let confirmed = acknowledged
        .iter()
        .find(|record| record["outcome"] == json!("confirmed"))
        .expect("the acknowledgement");
    assert_eq!(confirmed["message"], json!("Acknowledge"));
    assert_eq!(confirmed["user"], json!("anna"));
    let refused = acknowledged
        .iter()
        .find(|record| record["outcome"] == json!("refused"))
        .expect("the refused action");
    assert_eq!(refused["message"], json!("Take out of service"));
    assert!(
        refused["detail"]
            .as_str()
            .is_some_and(|reason| reason.contains("Engineer")),
        "{refused}"
    );
    assert_eq!(
        acknowledged.len(),
        2,
        "one record per command, its final outcome: {acknowledged:?}"
    );
    let trip = records(&alarms, "Tank.Trip", "command");
    assert_eq!(trip.len(), 1, "{trip:?}");
    assert_eq!(trip[0]["message"], json!("Write true"));
    assert_eq!(trip[0]["outcome"], json!("confirmed"));
    assert_eq!(trip[0]["user"], json!("anna"));
}

/// Specification 40, §4 and §5 item 3: a comment has 1 to 120 characters in any language, counted
/// as the UTF-16 code units of `Request.Comment`, a `WSTRING[120]`. A comment of 120 "ä" (240 bytes
/// of UTF-8) is applied and recorded whole; one of 121 "ä", and one of 61 "𝄞" (outside the Basic
/// Multilingual Plane, two code units each, 122 in all), are refused with the rule and nothing is
/// recorded. Catches a field that holds bytes, a limit counted in bytes or in characters, and a
/// comment cut to fit.
#[test]
fn a_comment_has_up_to_120_characters_in_any_language() {
    let plant = plant("alarms-comment-length");
    let full = "ä".repeat(120);
    let answer = plant.act(
        Some(&plant.anna),
        "c1",
        "Tank.High",
        "comment",
        Some(json!(full)),
    );
    assert_eq!(answer["outcome"], json!("applied"), "{answer}");
    let alarms = plant.wait_for("the comment is recorded", |alarms| {
        !records(alarms, "Tank.High", "comment").is_empty()
    });
    assert_eq!(
        records(&alarms, "Tank.High", "comment")[0]["detail"],
        json!(full)
    );
    for (id, text) in [("c2", "ä".repeat(121)), ("c3", "𝄞".repeat(61))] {
        let answer = plant.act(
            Some(&plant.anna),
            id,
            "Tank.High",
            "comment",
            Some(json!(text)),
        );
        assert_eq!(answer["outcome"], json!("refused"), "{answer}");
        assert_eq!(
            answer["reason"],
            json!("a comment has 1 to 120 characters"),
            "{answer}"
        );
    }
    assert_eq!(
        records(&plant.alarms(), "Tank.High", "comment").len(),
        1,
        "only the comment that fits is recorded"
    );
}
