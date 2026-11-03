//! Specification 39, §5.8 and §12.8: the switch, selector, mode selector and slider send their
//! commands, and the runtime checks each value against the control on the page.

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::hmi::access::config::{hmi_access_from_runtime_toml, HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::security::AccessRole;

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

const PROGRAM: &str = r#"
TYPE Speeds :
(
    Off,
    Low,
    High
);
END_TYPE

TYPE Modes :
(
    Auto := 0,
    Manual := 1,
    Track := 2
);
END_TYPE

PROGRAM Main
VAR
    Fan : BOOL;
    Speed : Speeds;
    Stage : INT;
    Mode : Modes := Modes#Auto;
    Valve : REAL;
    Damper : REAL;
END_VAR
END_PROGRAM
"#;

const CONTROLS: &str = r#"format = 1
title = "Controls"

[[object]]
id = "fan"
type = "switch"
at = [0, 0]
size = [80, 40]

[object.bind]
value = { path = "Main.Fan", direction = "read-write" }

[[object]]
id = "speed"
type = "selector"
at = [0, 60]
size = [160, 40]
options = ["Off", "Low"]

[object.bind]
value = { path = "Main.Speed", direction = "read-write" }

[[object]]
id = "stage"
type = "selector"
at = [0, 120]
size = [160, 40]
options = ["Stop", "Slow", "Fast"]

[object.bind]
value = { path = "Main.Stage", direction = "read-write" }

[[object]]
id = "mode"
type = "mode-selector"
at = [0, 180]
size = [240, 40]
options = ["Manual", "Auto"]

[object.bind]
mode = { path = "Main.Mode", direction = "read-write" }

[[object]]
id = "valve"
type = "slider"
at = [0, 240]
size = [240, 40]
min = 0
max = 80

[object.bind]
value = { path = "Main.Valve", direction = "read-write" }

[[object]]
id = "damper"
type = "slider"
at = [0, 300]
size = [240, 40]

[object.bind]
value = { path = "Main.Damper", direction = "read-write" }
"#;

const POLICY: &str = r#"
[[runtime.web.hmi.write]]
path = "Main.*"
role = "operator"
"#;

struct Plant {
    running: Running,
    cookie: String,
    bundle: Value,
}

fn plant(name: &str) -> Plant {
    let root = project(
        name,
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/controls.toml", CONTROLS),
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
    let running = start_with(&root, WebAuthMode::Local, None, access, PROGRAM, None);
    let cookie = session_cookie(&running.base, "anna");
    let page = http_request(
        "GET",
        &running.base,
        "/api/hmi/page?name=controls",
        None,
        None,
    )
    .1;
    let bundle = page["bundle"].clone();
    Plant {
        running,
        cookie,
        bundle,
    }
}

impl Plant {
    fn write(&self, id: &str, object: &str, value: Value) -> Value {
        let body = json!({ "id": id, "bundle": self.bundle, "page": "controls", "object": object, "action": "write", "value": value });
        self.send(body)
    }

    /// A command with what the control showed (§5.8): `shown` is left out when `None`.
    fn command(
        &self,
        id: &str,
        object: &str,
        action: &str,
        value: Option<Value>,
        shown: Option<Value>,
    ) -> Value {
        let mut body = json!({ "id": id, "bundle": self.bundle, "page": "controls", "object": object, "action": action });
        if let Some(value) = value {
            body["value"] = value;
        }
        if let Some(shown) = shown {
            body["shown"] = shown;
        }
        self.send(body)
    }

    fn send(&self, body: Value) -> Value {
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
}

fn applied(answer: &Value, readback: Value) {
    assert_eq!(answer["outcome"], json!("applied"), "{answer}");
    assert_eq!(answer["readback"], readback, "{answer}");
}

fn refused(answer: &Value, words: &str) {
    assert_eq!(answer["outcome"], json!("refused"), "{answer}");
    let reason = answer["reason"].as_str().unwrap_or_default();
    assert!(reason.contains(words), "{words}: {answer}");
}

#[test]
fn a_switch_writes_true_or_false() {
    let plant = plant("controls-switch");
    applied(&plant.write("s1", "fan", json!(true)), json!(true));
    refused(&plant.write("s2", "fan", json!("on")), "TRUE or FALSE");
}

/// A switch toggles to the opposite of the value it shows (specification 39 §5.8; the owner decided
/// on 1 November 2026 that the runtime decides it): FALSE shown writes TRUE, TRUE shown writes FALSE.
/// While it shows no value, `shown` is null or left out, the runtime refuses with the words the page
/// says in its language. Catches a toggle of the runtime's own value, which acts against what a
/// stale screen showed, and a toggle written blind.
#[test]
fn a_switch_writes_the_opposite_of_the_value_it_shows() {
    let plant = plant("controls-toggle");
    applied(
        &plant.command("t1", "fan", "toggle", None, Some(json!(false))),
        json!(true),
    );
    applied(
        &plant.command("t2", "fan", "toggle", None, Some(json!(false))),
        json!(true),
    );
    applied(
        &plant.command("t3", "fan", "toggle", None, Some(json!(true))),
        json!(false),
    );
    for (id, shown) in [("t4", Some(json!(null))), ("t5", None)] {
        refused(
            &plant.command(id, "fan", "toggle", None, shown),
            "The value of this control is not known, so it sends nothing.",
        );
    }
    refused(
        &plant.command("t6", "fan", "toggle", None, Some(json!("on"))),
        "a switch shows TRUE or FALSE",
    );
}

/// A control that acts on the value it shows sends nothing while it shows none (§5.8): a write
/// whose `shown` is null is refused, and one whose sender leaves `shown` out is checked as before.
/// Catches a selector, mode selector or slider command written while the page showed no value.
#[test]
fn a_control_that_shows_no_value_writes_nothing() {
    let plant = plant("controls-not-known");
    for (id, object, value) in [
        ("n1", "speed", json!("low")),
        ("n2", "mode", json!("Manual")),
        ("n3", "valve", json!(20.0)),
        ("n4", "fan", json!(true)),
    ] {
        refused(
            &plant.command(id, object, "write", Some(value), Some(json!(null))),
            "is not known",
        );
    }
    applied(
        &plant.command("n5", "speed", "write", Some(json!("low")), None),
        json!("Low"),
    );
}

#[test]
fn a_selector_writes_one_of_its_options_by_name_or_by_number() {
    let plant = plant("controls-selector");
    applied(&plant.write("c1", "speed", json!("low")), json!("Low"));
    refused(
        &plant.write("c2", "speed", json!("High")),
        "not one of its options",
    );
    applied(&plant.write("c3", "stage", json!("Fast")), json!(2));
    refused(
        &plant.write("c4", "stage", json!(1)),
        "not one of its options",
    );
}

#[test]
fn a_mode_selector_writes_one_of_the_modes_it_offers() {
    let plant = plant("controls-mode");
    applied(&plant.write("m1", "mode", json!("manual")), json!("Manual"));
    refused(
        &plant.write("m2", "mode", json!("Track")),
        "not one of its modes",
    );
}

#[test]
fn a_slider_writes_within_its_limits_which_default_to_0_to_100() {
    let plant = plant("controls-slider");
    applied(&plant.write("v1", "valve", json!(40.0)), json!(40.0));
    refused(
        &plant.write("v2", "valve", json!(85.0)),
        "above the highest allowed value 80",
    );
    refused(
        &plant.write("d1", "damper", json!(120.0)),
        "above the highest allowed value 100",
    );
    refused(
        &plant.write("d2", "damper", json!(-5.0)),
        "below the lowest allowed value 0",
    );
}
