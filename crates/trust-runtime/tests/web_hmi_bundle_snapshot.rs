//! Specification 39, §21.5 item 4: requests use the active bundle's snapshot, which is taken again
//! when a deploy or a rollback activates another bundle and when the files are changed on disk,
//! and at no other time. That the snapshot is not read again for a request is shown by the
//! snapshot's own unit tests; here the running runtime's page version follows each activation and
//! hand edit at once, and a page drawn before a change is out of date.

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::hmi::access::config::{hmi_access_from_runtime_toml, HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::hmi::bundle::HmiBundle;
use trust_runtime::security::AccessRole;

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

const PROGRAM: &str = "PROGRAM Main\nVAR\n    Setpoint : REAL := 50.0;\nEND_VAR\nEND_PROGRAM\n";

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

fn version(base: &str) -> Value {
    http_request("GET", base, "/api/hmi/page?name=overview", None, None).1["bundle"].clone()
}

fn command(base: &str, cookie: &str, id: &str, bundle: &Value) -> Value {
    let body = json!({ "id": id, "bundle": bundle, "page": "overview", "object": "sp", "action": "write", "value": 10 });
    let (status, answer, _) =
        http_request("POST", base, "/api/hmi/command", Some(cookie), Some(body));
    assert_eq!(status, 200, "{answer}");
    answer
}

#[test]
fn the_page_version_follows_each_activation_and_hand_edit_at_once() {
    let root = project("snapshot", &[]);
    bundle("First").write(&root).expect("the first bundle");
    let access = HmiAccessConfig {
        users: vec![HmiUser {
            name: "anna".into(),
            role: AccessRole::Operator,
            password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
                .expect("hash"),
        }],
        write: hmi_access_from_runtime_toml(
            "[[runtime.web.hmi.write]]\npath = \"Main.Setpoint\"\nrole = \"operator\"\n",
        )
        .expect("the policy")
        .write,
        ..HmiAccessConfig::default()
    };
    let running = start_with(&root, WebAuthMode::Local, None, access, PROGRAM, None);
    let base = running.base.as_str();
    let first = version(base);
    assert_eq!(first, json!(bundle("First").short_version()));
    for _ in 0..5 {
        assert_eq!(version(base), first, "nothing changed");
    }

    let second = bundle("Second");
    let (status, answer, _) = http_request(
        "POST",
        base,
        "/api/hmi/deploy",
        None,
        Some(second.to_json()),
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(version(base), json!(second.short_version()), "deployed");

    let (status, answer, _) =
        http_request("POST", base, "/api/hmi/rollback", None, Some(json!({})));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(version(base), first, "rolled back");

    // An edit on the runtime's own disk, as an engineer working on the machine makes it.
    std::fs::write(root.join("hmi/pages/overview.toml"), page("Edited by hand")).expect("the edit");
    let edited = version(base);
    assert_ne!(edited, first);
    assert_eq!(
        edited,
        json!(HmiBundle::read(&root).expect("readable").short_version()),
        "the files on disk now"
    );

    let cookie = session_cookie(base, "anna");
    let stale = command(base, &cookie, "c-stale", &first);
    assert_eq!(stale["outcome"], json!("refused"), "{stale}");
    assert!(
        stale["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("reload")),
        "{stale}"
    );
    let fresh = command(base, &cookie, "c-fresh", &edited);
    assert_eq!(fresh["outcome"], json!("applied"), "{fresh}");
}
