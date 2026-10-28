//! Specification 39, §21.5 item 3: every HMI write route (login, logout, command, alarm, settings,
//! deploy, rollback) requires `Content-Type: application/json` and, when the request carries
//! `Origin`, the runtime's own origin. Otherwise it answers 403 before it has any effect.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::control::ControlAuditEvent;
use trust_runtime::hmi::access::config::{hmi_access_from_runtime_toml, HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::hmi::bundle::HmiBundle;
use trust_runtime::security::AccessRole;

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

const PROGRAM: &str = "PROGRAM Main\nVAR\n    Setpoint : REAL := 50.0;\nEND_VAR\nEND_PROGRAM\n";

const POLICY: &str = "[[runtime.web.hmi.write]]\npath = \"Main.Setpoint\"\nrole = \"operator\"\n";

const JSON: (&str, &str) = ("Content-Type", "application/json");
const CROSS_SITE: &[(&str, &str)] = &[JSON, ("Origin", "http://attacker.example")];
const NOT_JSON: &[(&str, &str)] = &[("Content-Type", "text/plain")];

fn page(title: &str) -> String {
    format!("format = 1\ntitle = \"{title}\"\n\n[[object]]\nid = \"sp\"\ntype = \"setpoint\"\nat = [0, 0]\nsize = [200, 48]\n\n[object.bind]\nvalue = {{ path = \"Main.Setpoint\", direction = \"read-write\" }}\n")
}

fn bundle(title: &str) -> HmiBundle {
    HmiBundle {
        files: [
            ("hmi/screen.toml".to_string(), SCREEN.as_bytes().to_vec()),
            (
                "hmi/pages/overview.toml".to_string(),
                page(title).into_bytes(),
            ),
        ]
        .into_iter()
        .collect(),
    }
}

struct Plant {
    running: Running,
    audit: mpsc::Receiver<ControlAuditEvent>,
}

fn plant(name: &str) -> Plant {
    let root = project(name, &[]);
    bundle("First").write(&root).expect("the bundle");
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
    Plant {
        running: start_with(
            &root,
            WebAuthMode::Local,
            None,
            access,
            PROGRAM,
            Some(audit_tx),
        ),
        audit,
    }
}

impl Plant {
    fn base(&self) -> &str {
        &self.running.base
    }

    /// A request from the runtime's own page: JSON with the runtime's origin.
    fn same_origin(&self) -> Vec<(&str, &str)> {
        vec![JSON, ("Origin", self.base())]
    }

    /// A POST with exactly these headers (and a cookie): its status, JSON answer and cookie.
    fn post(
        &self,
        path: &str,
        body: &Value,
        headers: &[(&str, &str)],
        cookie: Option<&str>,
    ) -> (u16, Value, String) {
        let mut request = ureq::post(&format!("{}{path}", self.base()))
            .config()
            .http_status_as_error(false)
            .build();
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        if let Some(cookie) = cookie {
            request = request.header("Cookie", cookie);
        }
        let mut response = request
            .send(body.to_string())
            .unwrap_or_else(|error| panic!("POST {path}: {error}"));
        let status = response.status().as_u16();
        let set_cookie = response
            .headers()
            .get("Set-Cookie")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let text = response.body_mut().read_to_string().unwrap_or_default();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::Null),
            set_cookie,
        )
    }

    /// Asserts that a cross-site and a non-JSON request are refused by the check itself.
    fn refuses(&self, path: &str, body: &Value, cookie: Option<&str>) {
        for headers in [CROSS_SITE, NOT_JSON] {
            let (status, answer, set_cookie) = self.post(path, body, headers, cookie);
            assert_eq!(status, 403, "{path} {headers:?}: {answer}");
            assert!(answer["denial_code"].is_string(), "{path}: {answer}");
            assert!(set_cookie.is_empty(), "{path}: no cookie: {set_cookie}");
        }
    }

    fn login(&self, user: &str) -> String {
        let (status, answer, set_cookie) = self.post(
            "/api/hmi/login",
            &json!({ "user": user, "password": "secret" }),
            &self.same_origin(),
            None,
        );
        assert_eq!(status, 200, "{answer}");
        set_cookie.split(';').next().unwrap_or_default().to_string()
    }

    fn version(&self) -> Value {
        http_request(
            "GET",
            self.base(),
            "/api/hmi/page?name=overview",
            None,
            None,
        )
        .1["bundle"]
            .clone()
    }

    /// The audit records of these kinds that arrive within a second.
    fn audited(&self, kinds: &[&str]) -> Vec<Value> {
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut records = Vec::new();
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match self.audit.recv_timeout(left) {
                Ok(event) if kinds.contains(&event.request_type.as_str()) => {
                    records.push(event.details.unwrap_or(Value::Null));
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        records
    }
}

#[test]
fn login_refuses_a_cross_site_or_non_json_request_without_counting_it() {
    let plant = plant("policy-login");
    for _ in 0..5 {
        plant.refuses(
            "/api/hmi/login",
            &json!({ "user": "erik", "password": "wrong" }),
            None,
        );
    }
    plant.refuses(
        "/api/hmi/login",
        &json!({ "user": "erik", "password": "secret" }),
        None,
    );
    // Five counted failures would refuse erik for a minute.
    let cookie = plant.login("erik");
    assert!(cookie.starts_with("trust_hmi_session="), "{cookie}");
}

#[test]
fn logout_refuses_a_cross_site_or_non_json_request_and_the_session_stays() {
    let plant = plant("policy-logout");
    let cookie = plant.login("anna");
    plant.refuses("/api/hmi/logout", &json!({}), Some(&cookie));
    let (_, session, _) =
        http_request("GET", plant.base(), "/api/hmi/session", Some(&cookie), None);
    assert_eq!(session["user"], json!("anna"), "{session}");
    let (status, _, _) = plant.post(
        "/api/hmi/logout",
        &json!({}),
        &plant.same_origin(),
        Some(&cookie),
    );
    assert_eq!(status, 200);
    let (_, ended, _) = http_request("GET", plant.base(), "/api/hmi/session", Some(&cookie), None);
    assert_eq!(ended["session"], Value::Null, "{ended}");
}

#[test]
fn a_command_is_refused_cross_site_or_not_json_before_it_is_executed() {
    let plant = plant("policy-command");
    let cookie = plant.login("anna");
    let bundle = plant.version();
    let command = |id: &str, value: f64| json!({ "id": id, "bundle": bundle, "page": "overview", "object": "sp", "action": "write", "value": value });
    plant.refuses(
        "/api/hmi/command",
        &command("c-shared", 61.5),
        Some(&cookie),
    );
    plant.refuses(
        "/api/hmi/command",
        &command("c-refused", 62.5),
        Some(&cookie),
    );
    // The id was never taken: the same id from the page is executed as new.
    let (status, answer, _) = plant.post(
        "/api/hmi/command",
        &command("c-shared", 70.0),
        &plant.same_origin(),
        Some(&cookie),
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        (&answer["outcome"], &answer["readback"]),
        (&json!("applied"), &json!(70.0)),
        "{answer}"
    );
    let records = plant.audited(&["hmi.command"]);
    assert!(
        records
            .iter()
            .any(|record| record["command"] == json!("c-shared")),
        "{records:?}"
    );
    assert!(
        !records
            .iter()
            .any(|record| record["command"] == json!("c-refused")),
        "a refused request leaves no record: {records:?}"
    );
}

#[test]
fn an_alarm_action_is_refused_cross_site_or_not_json_before_it_is_executed() {
    let plant = plant("policy-alarm");
    let cookie = plant.login("anna");
    let action = |id: &str| json!({ "id": id, "alarm": "Main.Overflow", "action": "acknowledge" });
    plant.refuses("/api/hmi/alarm", &action("a-refused"), Some(&cookie));
    let (status, answer, _) = plant.post(
        "/api/hmi/alarm",
        &action("a-page"),
        &plant.same_origin(),
        Some(&cookie),
    );
    assert_eq!(status, 200, "the route itself answers: {answer}");
    let records = plant.audited(&["hmi.alarm"]);
    assert!(
        records
            .iter()
            .any(|record| record["command"] == json!("a-page")),
        "{records:?}"
    );
    assert!(
        !records
            .iter()
            .any(|record| record["command"] == json!("a-refused")),
        "a refused request leaves no record: {records:?}"
    );
}

#[test]
fn settings_refuse_a_cross_site_or_non_json_request() {
    let plant = plant("policy-settings");
    let cookie = plant.login("erik");
    let read = json!({ "faceplate": "overview/sp", "action": "read" });
    plant.refuses("/api/hmi/settings", &read, Some(&cookie));
    let (_, answer, _) = plant.post(
        "/api/hmi/settings",
        &read,
        &plant.same_origin(),
        Some(&cookie),
    );
    assert!(
        answer.get("denial_code").is_none(),
        "the route itself answers: {answer}"
    );
}

#[test]
fn deploy_and_rollback_refuse_a_cross_site_or_non_json_request_without_effect() {
    let plant = plant("policy-deploy");
    let first = plant.version();
    assert_eq!(first, json!(bundle("First").short_version()));
    let second = bundle("Second");
    plant.refuses("/api/hmi/deploy", &second.to_json(), None);
    assert_eq!(plant.version(), first, "nothing was activated");
    let (status, answer, _) = plant.post(
        "/api/hmi/deploy",
        &second.to_json(),
        &plant.same_origin(),
        None,
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(plant.version(), json!(second.short_version()));
    let deployed = plant.audited(&["hmi.deploy", "hmi.rollback"]);
    assert_eq!(
        deployed.len(),
        1,
        "only the deploy from the page is audited: {deployed:?}"
    );
    plant.refuses("/api/hmi/rollback", &json!({}), None);
    assert_eq!(
        plant.version(),
        json!(second.short_version()),
        "nothing was rolled back"
    );
    assert!(
        plant.audited(&["hmi.deploy", "hmi.rollback"]).is_empty(),
        "a refused rollback leaves no record"
    );
    let (status, answer, _) =
        plant.post("/api/hmi/rollback", &json!({}), &plant.same_origin(), None);
    assert_eq!(status, 200, "{answer}");
    assert_eq!(plant.version(), first);
}

#[test]
fn a_program_without_origin_such_as_the_editor_may_write() {
    let plant = plant("policy-no-origin");
    let (status, answer, set_cookie) = plant.post(
        "/api/hmi/login",
        &json!({ "user": "anna", "password": "secret" }),
        &[JSON],
        None,
    );
    assert_eq!(status, 200, "{answer}");
    assert!(set_cookie.contains("trust_hmi_session="), "{set_cookie}");
    let (status, answer, _) = plant.post(
        "/api/hmi/deploy",
        &bundle("Second").to_json(),
        &[JSON],
        None,
    );
    assert_eq!(status, 200, "{answer}");
}
