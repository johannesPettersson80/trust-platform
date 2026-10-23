//! Specification 39, §16.2 and §16.4: deploying an HMI bundle to a running runtime: the checks,
//! one-step activation, refusal that keeps the previous bundle, rollback, old browsers, the
//! deploy permission and the audit log.

use std::net::TcpStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::control::ControlAuditEvent;
use trust_runtime::hmi::access::config::{hmi_access_from_runtime_toml, HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::hmi::bundle::HmiBundle;
use trust_runtime::security::AccessRole;
use tungstenite::Message;

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

const PROGRAM: &str =
    "PROGRAM Main\nVAR\n    Level : REAL;\n    Setpoint : REAL;\nEND_VAR\nEND_PROGRAM\n";

fn page(title: &str, path: &str) -> String {
    format!("format = 1\ntitle = \"{title}\"\n\n[[object]]\nid = \"level\"\ntype = \"value\"\nat = [0, 0]\nsize = [160, 48]\n\n[object.bind]\nvalue = \"{path}\"\n")
}

fn bundle(title: &str, path: &str) -> HmiBundle {
    HmiBundle {
        files: [
            ("hmi/screen.toml".to_string(), SCREEN.as_bytes().to_vec()),
            (
                "hmi/pages/overview.toml".to_string(),
                page(title, path).into_bytes(),
            ),
        ]
        .into_iter()
        .collect(),
    }
}

struct Plant {
    running: Running,
    root: std::path::PathBuf,
    _audit: mpsc::Receiver<ControlAuditEvent>,
}

fn plant_root(plant: &Plant) -> std::path::PathBuf {
    plant.root.clone()
}

/// The runtime configuration a program deployment's preflight reads.
const RUNTIME_TOML: &str = "[bundle]\nversion = 1\n\n[resource]\nname = \"Cpu\"\ncycle_interval_ms = 100\n\n[runtime.control]\nendpoint = \"tcp://127.0.0.1:0\"\nmode = \"production\"\ndebug_enabled = false\nauth_token = \"t\"\n\n[runtime.log]\nlevel = \"info\"\n\n[runtime.retain]\nmode = \"none\"\nsave_interval_ms = 1000\n\n[runtime.watchdog]\nenabled = false\ntimeout_ms = 5000\naction = \"halt\"\n\n[runtime.fault]\npolicy = \"halt\"\n\n[runtime.web]\nenabled = true\nlisten = \"127.0.0.1:0\"\nauth = \"local\"\ntls = false\n\n[runtime.tls]\nmode = \"disabled\"\n";

fn plant(name: &str, auth: WebAuthMode, token: Option<&str>) -> Plant {
    let root = project(
        name,
        &[
            ("runtime.toml", RUNTIME_TOML),
            ("io.toml", "[io]\ndriver = \"simulated\"\nparams = {}\n"),
        ],
    );
    bundle("First", "Main.Level")
        .write(&root)
        .expect("the first bundle");
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
        write: hmi_access_from_runtime_toml(
            "[[runtime.web.hmi.write]]\npath = \"Main.Setpoint\"\nrole = \"operator\"\n",
        )
        .expect("policy")
        .write,
        ..HmiAccessConfig::default()
    };
    let (audit_tx, audit) = mpsc::channel();
    Plant {
        running: start_with(&root, auth, token, access, PROGRAM, Some(audit_tx)),
        root,
        _audit: audit,
    }
}

fn post(base: &str, path: &str, body: Value, headers: &[(&str, &str)]) -> (u16, Value, String) {
    // The HMI write routes take JSON only (specification 39, §21.5 item 3).
    let mut request = ureq::post(&format!("{base}{path}"))
        .config()
        .http_status_as_error(false)
        .build()
        .header("Content-Type", "application/json");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = request
        .send(body.to_string())
        .unwrap_or_else(|error| panic!("POST {path}: {error}"));
    let status = response.status().as_u16();
    let cookie = response
        .headers()
        .get("Set-Cookie")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .to_string();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    (
        status,
        serde_json::from_str(&text).unwrap_or(Value::Null),
        cookie,
    )
}

fn page_data(base: &str, headers: &[(&str, &str)]) -> Value {
    let mut request = ureq::get(&format!("{base}/api/hmi/page?name=overview"))
        .config()
        .http_status_as_error(false)
        .build();
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = request.call().expect("page data");
    serde_json::from_str(&response.body_mut().read_to_string().unwrap_or_default()).expect("JSON")
}

/// Specification 39, §5.9 item 1: after a deployment the runtime keeps the values the new bundle's
/// trends show.
#[test]
fn after_a_deployment_the_new_bundles_trended_values_are_kept() {
    let plant = plant("deploy-trend", WebAuthMode::Local, None);
    let samples = |plant: &Plant| {
        http_request(
            "GET",
            &plant.running.base,
            "/api/hmi/history?path=Main.Level&span=60",
            None,
            None,
        )
        .1["samples"]["Main.Level"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    };
    assert!(samples(&plant).is_empty(), "the first bundle has no trend");
    let trend = "format = 1\ntitle = \"Trend\"\n\n[[object]]\nid = \"trend\"\ntype = \"trend\"\nat = [0, 0]\nsize = [600, 300]\n\n[object.bind]\nlevel = \"Main.Level\"\n";
    let trended = HmiBundle {
        files: [
            ("hmi/screen.toml".to_string(), SCREEN.as_bytes().to_vec()),
            (
                "hmi/pages/overview.toml".to_string(),
                trend.as_bytes().to_vec(),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let (status, answer, _) = post(
        &plant.running.base,
        "/api/hmi/deploy",
        trended.to_json(),
        &[],
    );
    assert_eq!(status, 200, "{answer}");
    std::thread::sleep(std::time::Duration::from_millis(1_500));
    assert!(!samples(&plant).is_empty(), "kept since the deployment");
}

#[test]
fn a_valid_bundle_activates_in_one_step() {
    let plant = plant("deploy-valid", WebAuthMode::Local, None);
    let second = bundle("Second", "Main.Level");
    let (status, answer, _) = post(
        &plant.running.base,
        "/api/hmi/deploy",
        second.to_json(),
        &[],
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["active"], json!(second.short_version()), "{answer}");
    assert_eq!(
        answer["previous"],
        json!(bundle("First", "Main.Level").short_version()),
        "{answer}"
    );
    let data = page_data(&plant.running.base, &[]);
    assert_eq!(data["page"]["model"]["title"], json!("Second"));
    assert_eq!(data["bundle"], json!(second.short_version()));
}

#[test]
fn an_invalid_bundle_is_refused_and_the_previous_one_keeps_running() {
    let plant = plant("deploy-invalid", WebAuthMode::Local, None);
    let (status, answer, _) = post(
        &plant.running.base,
        "/api/hmi/deploy",
        bundle("Broken", "Main.Gone").to_json(),
        &[],
    );
    assert_eq!(status, 422, "{answer}");
    let problem = &answer["problems"][0];
    assert_eq!(
        (&problem["file"], &problem["line"]),
        (&json!("hmi/pages/overview.toml"), &json!(11)),
        "{answer}"
    );
    assert!(
        problem["message"]
            .as_str()
            .is_some_and(|message| message.contains("Main.Gone")),
        "{answer}"
    );
    assert_eq!(
        page_data(&plant.running.base, &[])["page"]["model"]["title"],
        json!("First")
    );
    let mut tampered = bundle("Second", "Main.Level").to_json();
    tampered["files"]["hmi/pages/overview.toml"] = json!(page("Tampered", "Main.Level"));
    let (status, refused, _) = post(&plant.running.base, "/api/hmi/deploy", tampered, &[]);
    assert_eq!(status, 400, "{refused}");
    assert!(
        refused["error"]
            .as_str()
            .is_some_and(|error| error.contains("version")),
        "{refused}"
    );
}

#[test]
fn a_rollback_restores_the_previous_bundle() {
    let plant = plant("deploy-rollback", WebAuthMode::Local, None);
    post(
        &plant.running.base,
        "/api/hmi/deploy",
        bundle("Second", "Main.Level").to_json(),
        &[],
    );
    let (status, answer, _) = post(&plant.running.base, "/api/hmi/rollback", json!({}), &[]);
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer["active"],
        json!(bundle("First", "Main.Level").short_version())
    );
    assert_eq!(
        page_data(&plant.running.base, &[])["page"]["model"]["title"],
        json!("First")
    );
}

#[test]
fn a_browser_showing_the_old_bundle_is_told_to_reload() {
    let plant = plant("deploy-reload", WebAuthMode::Local, None);
    let address = plant.running.base.trim_start_matches("http://").to_string();
    let stream = TcpStream::connect(&address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_millis(300)))
        .expect("timeout");
    let (mut socket, _) =
        tungstenite::client(format!("ws://{address}/ws/hmi?page=overview"), stream)
            .expect("the stream opens");
    let second = bundle("Second", "Main.Level");
    post(
        &plant.running.base,
        "/api/hmi/deploy",
        second.to_json(),
        &[],
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut told = None;
    while Instant::now() < deadline && told.is_none() {
        if let Ok(Message::Text(text)) = socket.read() {
            let message: Value = serde_json::from_str(text.as_str()).expect("JSON");
            told = message.get("bundle").cloned();
        }
    }
    assert_eq!(told, Some(json!(second.short_version())));
}

#[test]
fn deploying_needs_the_deploy_permission_and_is_audited() {
    let root = project("deploy-permission", &[]);
    bundle("First", "Main.Level")
        .write(&root)
        .expect("the first bundle");
    // A paired browser with the Viewer role: web access, but no deploy permission by itself.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    std::fs::write(
        root.join("pairings.json"),
        json!({ "tokens": [{ "id": "pair-1", "token": "viewer-token", "created_at": now, "enabled": true, "role": "viewer", "expires_at": now + 86_400 }] }).to_string(),
    )
    .expect("pairings");
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
        ..HmiAccessConfig::default()
    };
    let (audit_tx, audit) = mpsc::channel();
    let running = start_with(
        &root,
        WebAuthMode::Token,
        Some("admin-token"),
        access,
        PROGRAM,
        Some(audit_tx),
    );
    let base = running.base.as_str();
    let viewer = ("X-Trust-Token", "viewer-token");
    let login = |user: &str| {
        post(
            base,
            "/api/hmi/login",
            json!({ "user": user, "password": "secret" }),
            &[viewer],
        )
        .2
    };
    let (operator, engineer) = (login("anna"), login("erik"));
    let second = bundle("Second", "Main.Level");
    assert_eq!(
        post(base, "/api/hmi/deploy", second.to_json(), &[]).0,
        401,
        "no web access"
    );
    assert_eq!(
        post(base, "/api/hmi/deploy", second.to_json(), &[viewer]).0,
        403,
        "web access alone"
    );
    assert_eq!(
        post(
            base,
            "/api/hmi/deploy",
            second.to_json(),
            &[viewer, ("Cookie", operator.as_str())]
        )
        .0,
        403,
        "an Operator"
    );
    let (status, answer, _) = post(
        base,
        "/api/hmi/deploy",
        second.to_json(),
        &[viewer, ("Cookie", engineer.as_str())],
    );
    assert_eq!(status, 200, "an Engineer: {answer}");
    let (status, answer, _) = post(
        base,
        "/api/hmi/rollback",
        json!({}),
        &[("X-Trust-Token", "admin-token")],
    );
    assert_eq!(status, 200, "the administrator: {answer}");
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut records = Vec::new();
    while Instant::now() < deadline {
        if let Ok(event) = audit.recv_timeout(Duration::from_millis(100))
            && (event.request_type == "hmi.deploy" || event.request_type == "hmi.rollback")
        {
            records.push((
                event.request_type.to_string(),
                event.details.unwrap_or(Value::Null),
            ));
        }
    }
    let find = |kind: &str, outcome: &str| {
        records
            .iter()
            .find(|(seen, record)| seen == kind && record["outcome"] == json!(outcome))
            .cloned()
    };
    let activated = find("hmi.deploy", "activated").expect("the deploy is audited");
    assert_eq!(activated.1["who"], json!("erik"));
    assert_eq!(activated.1["version"], json!(second.short_version()));
    assert_eq!(
        activated.1["previous"],
        json!(bundle("First", "Main.Level").short_version())
    );
    assert_eq!(
        find("hmi.rollback", "activated")
            .expect("the rollback is audited")
            .1["who"],
        json!("administrator")
    );
    assert!(
        find("hmi.deploy", "refused").is_some(),
        "refused attempts are audited: {records:?}"
    );
}

const RENAMED: &str =
    "PROGRAM Main\nVAR\n    Height : REAL;\n    Setpoint : REAL;\nEND_VAR\nEND_PROGRAM\n";

fn project_deploy(base: &str, extra: Value) -> (u16, Value) {
    let mut body = json!({ "sources": [{ "path": "src/main.st", "content": RENAMED }] });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    let (status, answer, _) = post(base, "/api/deploy", body, &[]);
    (status, answer)
}

#[test]
fn a_program_deployment_that_would_break_bindings_lists_them_and_changes_nothing() {
    let plant = plant("program-breaks", WebAuthMode::Local, None);
    let (status, answer) = project_deploy(&plant.running.base, json!({}));
    assert_eq!(status, 409, "{answer}");
    let broken = &answer["broken"][0];
    assert_eq!(
        (&broken["file"], &broken["line"]),
        (&json!("hmi/pages/overview.toml"), &json!(11)),
        "{answer}"
    );
    assert!(
        broken["message"]
            .as_str()
            .is_some_and(|message| message.contains("Main.Level")),
        "{answer}"
    );
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("break")),
        "{answer}"
    );
    let root = plant_root(&plant);
    assert!(!root.join("src/main.st").exists(), "nothing was written");
}

#[test]
fn the_program_deploys_together_with_an_updated_bundle() {
    let plant = plant("program-with-hmi", WebAuthMode::Local, None);
    let updated = bundle("Updated", "Main.Height");
    let (status, answer) = project_deploy(&plant.running.base, json!({ "hmi": updated.to_json() }));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        answer["hmi"]["active"],
        json!(updated.short_version()),
        "{answer}"
    );
    let root = plant_root(&plant);
    assert!(root.join("src/main.st").exists(), "the program was written");
    assert_eq!(
        page_data(&plant.running.base, &[])["page"]["model"]["title"],
        json!("Updated")
    );
}

#[test]
fn accepting_broken_bindings_deploys_the_program_alone() {
    let plant = plant("program-accept", WebAuthMode::Local, None);
    let (status, answer) = project_deploy(
        &plant.running.base,
        json!({ "accept_broken_bindings": true }),
    );
    assert_eq!(status, 200, "{answer}");
    assert!(
        answer["broken"]
            .as_array()
            .is_some_and(|broken| broken.len() == 1),
        "the bindings that stay broken are named: {answer}"
    );
    assert!(plant_root(&plant).join("src/main.st").exists());
}
