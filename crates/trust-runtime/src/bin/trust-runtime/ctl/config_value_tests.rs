//! Specification 11, 6.8.1: what `ctl config-set` sends for its value, and its note on the keys
//! that take effect only at the next start (checklist G279).

use super::*;
use serde_json::json;

use crate::ctl::{build_request, ControlAction};

/// The WAN allowlist rule the federation guide writes.
const RULE: &str = r#"[{"action":"cfg_apply","target":"site-b/*"}]"#;

/// Catches a structured value sent as a string, which the runtime refuses for every array or
/// object key ("expected array of {action,target} objects" for the WAN allowlist, found by G245b).
#[test]
fn a_value_that_begins_with_a_bracket_or_a_brace_is_sent_as_its_json() {
    let request = build_request(
        &ControlAction::ConfigSet {
            key: "runtime_cloud.wan.allow_write".into(),
            value: format!(" {RULE} "),
        },
        Some("token"),
    );
    assert_eq!(
        request["params"],
        json!({ "runtime_cloud.wan.allow_write": [{"action": "cfg_apply", "target": "site-b/*"}] })
    );
    assert_eq!(
        parse_config_value(r#"["lo","eth0"]"#),
        json!(["lo", "eth0"])
    );
    assert_eq!(
        parse_config_value(r#"{"rest":"0.6"}"#),
        json!({"rest": "0.6"})
    );
    assert_eq!(parse_config_value("[]"), json!([]));
    assert_eq!(
        parse_config_value("debug"),
        json!("debug"),
        "other text stays a string"
    );
}

/// Catches a mistyped structured value that reaches the runtime as a string, where its type error
/// hides the typo, or that is sent at all.
#[test]
fn a_bracketed_value_that_is_no_json_is_refused_before_anything_is_sent() {
    let refused = check_config_value("runtime_cloud.wan.allow_write", "[{action: cfg_apply}]")
        .expect_err("no JSON");
    let text = format!("{refused:#}");
    assert!(
        text.contains("config-set runtime_cloud.wan.allow_write") && text.contains("not JSON"),
        "{text}"
    );
    assert!(check_config_value("log.level", "debug").is_ok());
    assert!(check_config_value("discovery.interfaces", r#"["lo"]"#).is_ok());
    assert!(check_config_value("runtime_cloud.wan.allow_write", RULE).is_ok());
}

/// Catches a silent change that the next start undoes: `config.set` keeps a restart-required key
/// in memory only, and the start reads `runtime.toml`.
#[test]
fn the_keys_that_need_a_restart_are_named_with_runtime_toml() {
    let answer = json!({
        "id": 1,
        "ok": true,
        "result": {
            "updated": ["log.level", "runtime_cloud.wan.allow_write"],
            "restart_required": ["runtime_cloud.wan.allow_write"]
        }
    });
    assert_eq!(
        restart_required_note(&answer).as_deref(),
        Some(
            "config-set: runtime_cloud.wan.allow_write takes effect only after a restart, and the \
             next start reads runtime.toml, which config-set does not write: set the same value \
             there."
        )
    );
    let both = json!({"ok": true, "result": {"restart_required": ["mesh.listen", "web.listen"]}});
    assert!(
        restart_required_note(&both)
            .is_some_and(|note| note.starts_with("config-set: mesh.listen, web.listen take effect")),
        "both keys are named"
    );
    let in_place =
        json!({"ok": true, "result": {"updated": ["log.level"], "restart_required": []}});
    assert_eq!(
        restart_required_note(&in_place),
        None,
        "a key applied at once needs no note"
    );
}
