//! Specification 39, §12.6 and §21.5 item 1: the runtime ends every pulse. The end is written
//! after the pulse's duration even when the session ends and no browser is left, it waits for the
//! next display boundary instead of being dropped when the program does not run, and it goes to
//! the audit log (§12.7) with its outcome.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::control::ControlAuditEvent;
use trust_runtime::hmi::access::config::{hmi_access_from_runtime_toml, HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::security::AccessRole;

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

const PROGRAM: &str = r#"
PROGRAM Main
VAR
    Pulse : BOOL;
    Pulses : INT;
END_VAR
    IF Pulse THEN
        Pulses := Pulses + 1;
    END_IF;
END_PROGRAM
"#;

const OPERATE: &str = r#"format = 1
title = "Operate"

[[object]]
id = "pulse"
type = "pulse-button"
at = [0, 0]
size = [120, 40]
pulse = 0.3

[object.bind]
target = { path = "Main.Pulse", direction = "write" }

[[object]]
id = "pulse-state"
type = "status"
at = [140, 0]
size = [40, 40]

[object.bind]
state = "Main.Pulse"
"#;

const POLICY: &str = r#"
[[runtime.web.hmi.write]]
path = "Main.Pulse"
role = "operator"
"#;

struct Plant {
    running: Running,
    cookie: String,
    bundle: Value,
    audit: mpsc::Receiver<ControlAuditEvent>,
    /// The `hmi.command` audit records seen so far.
    records: Vec<Value>,
}

fn plant(name: &str) -> Plant {
    let root = project(
        name,
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/operate.toml", OPERATE),
        ],
    );
    let access = HmiAccessConfig {
        users: vec![HmiUser {
            name: "anna".into(),
            role: AccessRole::Operator,
            password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
                .expect("hash"),
        }],
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
    assert!(bundle.is_string(), "the page names its version: {page}");
    Plant {
        running,
        cookie,
        bundle,
        audit,
        records: Vec::new(),
    }
}

impl Plant {
    fn pulse(&self, id: &str) -> Value {
        let body = json!({ "id": id, "bundle": self.bundle, "page": "operate", "object": "pulse", "action": "pulse" });
        let (status, answer, _) = http_request(
            "POST",
            &self.running.base,
            "/api/hmi/command",
            Some(&self.cookie),
            Some(body),
        );
        assert_eq!(status, 200, "{answer}");
        answer
    }

    /// The point's value at the program's next display boundary.
    fn point(&self, path: &str) -> Value {
        let mut response = ureq::post(&format!("{}/api/control", self.running.base))
            .config()
            .http_status_as_error(false)
            .build()
            .header("Content-Type", "application/json")
            .send(json!({ "id": 1, "type": "hmi.read", "params": { "paths": [path] } }).to_string())
            .expect("the control request");
        let body: Value =
            serde_json::from_str(&response.body_mut().read_to_string().unwrap_or_default())
                .unwrap_or(Value::Null);
        body["result"]["values"][path]["value"].clone()
    }

    /// Waits up to `seconds` for the point to read `expected`.
    fn point_becomes(&self, path: &str, expected: &Value, seconds: u64) -> Value {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            let read = self.point(path);
            if &read == expected || Instant::now() > deadline {
                return read;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The audit record of a command's pulse end, waiting up to `wait` for it.
    fn pulse_end(&mut self, id: &str, wait: Duration) -> Option<Value> {
        let deadline = Instant::now() + wait;
        loop {
            let found = self
                .records
                .iter()
                .find(|record| record["command"] == json!(id) && record["pulse"] == json!("end"))
                .cloned();
            if found.is_some() {
                return found;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match self.audit.recv_timeout(left.max(Duration::from_millis(1))) {
                Ok(event) if event.request_type == "hmi.command" => {
                    self.records.push(event.details.unwrap_or(Value::Null));
                }
                Ok(_) => {}
                Err(_) if Instant::now() >= deadline => return None,
                Err(_) => {}
            }
        }
    }
}

#[test]
fn a_pulse_ends_after_its_duration_when_the_session_ends_and_no_browser_is_left() {
    let mut plant = plant("pulse-logout");
    assert_eq!(plant.pulse("c-gone")["outcome"], json!("applied"));
    // No page follows the command, and its user logs out before the pulse is over.
    let (status, _, _) = http_request(
        "POST",
        &plant.running.base,
        "/api/hmi/logout",
        Some(&plant.cookie),
        None,
    );
    assert_eq!(status, 200);
    let end = plant
        .pulse_end("c-gone", Duration::from_secs(5))
        .expect("the end is audited");
    assert_eq!(end["outcome"], json!("applied"), "{end}");
    assert_eq!(end["target"], json!("Main.Pulse"), "{end}");
    assert_eq!(
        (&end["old"], &end["requested"], &end["readback"]),
        (&json!(true), &json!(false), &json!(false)),
        "{end}"
    );
    assert_eq!(end["user"], json!("anna"), "{end}");
    assert!(
        end["session"].as_str().is_some_and(|id| !id.is_empty()),
        "{end}"
    );
    assert_eq!(end["bundle"], plant.bundle, "{end}");
    assert_eq!(end["attempts"], json!(1), "{end}");
    assert!(end["time_ms"].as_u64().is_some(), "{end}");
    assert_eq!(plant.point("Main.Pulse"), json!(false));
    assert!(
        plant
            .point("Main.Pulses")
            .as_i64()
            .is_some_and(|scans| scans > 0),
        "it was on for some scans"
    );
}

#[test]
fn a_pulse_end_due_while_the_program_is_paused_is_written_when_it_runs_again() {
    let mut plant = plant("pulse-paused");
    let resource = plant.running.handle.control();
    assert_eq!(plant.pulse("c-paused")["outcome"], json!("applied"));
    resource.pause().expect("the program pauses");
    // Past the pulse's end and past the time a request waits for the program.
    std::thread::sleep(Duration::from_millis(300 + 2_500));
    assert!(
        plant.pulse_end("c-paused", Duration::ZERO).is_none(),
        "no display boundary has run, so nothing was written yet"
    );
    resource.resume().expect("the program runs again");
    assert_eq!(
        plant.point_becomes("Main.Pulse", &json!(false), 5),
        json!(false),
        "the end was kept, not dropped"
    );
    let end = plant
        .pulse_end("c-paused", Duration::from_secs(3))
        .expect("the end is audited");
    assert_eq!(end["outcome"], json!("applied"), "{end}");
    assert_eq!(
        end["attempts"],
        json!(1),
        "written at the first boundary after: {end}"
    );
    assert_eq!(end["readback"], json!(false), "{end}");
}
