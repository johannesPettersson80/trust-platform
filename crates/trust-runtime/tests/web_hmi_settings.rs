//! Specification 39, §21.4 and Plant library §10: the Settings dialog's route for the real Plant
//! pump. An Engineer reads the settings the pump's component definition describes, in their
//! units, and applies changes, which are checked against that description and written together at
//! one boundary, or not at all.

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::harness::CompileSession;
use trust_runtime::hmi::access::config::{HmiAccessConfig, HmiUser};
use trust_runtime::hmi::access::password::{hash_password, PasswordHash};
use trust_runtime::security::AccessRole;
use trust_runtime::RetainPolicy;

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
"#;

/// The pump's panel, as the dialog names it.
const PANEL: &str = "faceplate:overview/pump";

fn user(name: &str, role: AccessRole) -> HmiUser {
    HmiUser {
        name: name.into(),
        role,
        password: PasswordHash::parse(&hash_password("secret", 10_000, b"0123456789abcdef"))
            .expect("hash"),
    }
}

/// The pump's project with operator anna and engineer erik.
fn start_plant(name: &str) -> Running {
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
        ],
        ..HmiAccessConfig::default()
    };
    let mut programs = SOURCES.to_vec();
    programs.push(PROGRAM);
    start_with_sources(&root, WebAuthMode::Local, None, access, &programs, None)
}

fn settings(running: &Running, cookie: &str, body: Value) -> Value {
    let (status, answer, _) = http_request(
        "POST",
        &running.base,
        "/api/hmi/settings",
        Some(cookie),
        Some(body),
    );
    assert_eq!(status, 200, "{answer}");
    answer
}

fn read(running: &Running, cookie: &str) -> Value {
    settings(
        running,
        cookie,
        json!({ "faceplate": PANEL, "action": "read" }),
    )
}

fn setting<'a>(answer: &'a Value, member: &str) -> &'a Value {
    answer["settings"]
        .as_array()
        .and_then(|settings| settings.iter().find(|setting| setting["member"] == member))
        .unwrap_or_else(|| panic!("{member} in {answer}"))
}

fn number(answer: &Value, member: &str) -> Option<f64> {
    setting(answer, member)["value"].as_f64()
}

#[test]
fn an_engineer_reads_the_settings_with_titles_units_and_values_from_the_definition() {
    let running = start_plant("settings-read");
    let erik = session_cookie(&running.base, "erik");
    let answer = read(&running, &erik);
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(answer["instance"], json!("Main.P101"));
    let members: Vec<&str> = answer["settings"]
        .as_array()
        .expect("settings")
        .iter()
        .filter_map(|setting| setting["member"].as_str())
        .collect();
    assert_eq!(
        members,
        [
            "HasFeedback",
            "StartTimeout",
            "StopTimeout",
            "ServiceInterval"
        ],
        "in the definition's order"
    );
    let feedback = setting(&answer, "HasFeedback");
    assert_eq!(feedback["title"], json!("Has running feedback"));
    assert_eq!(
        feedback["value"],
        json!(false),
        "as the declaration sets it"
    );
    let timeout = setting(&answer, "StartTimeout");
    assert_eq!(timeout["title"], json!("Start timeout"));
    assert_eq!(timeout["unit"], json!("s"));
    assert_eq!(timeout["kind"], json!("time"));
    assert_eq!(
        (timeout["min"].as_f64(), timeout["max"].as_f64()),
        (Some(1.0), Some(600.0))
    );
    assert_eq!(
        timeout["help"],
        json!("Running feedback must arrive within this time.")
    );
    assert_eq!(
        number(&answer, "StartTimeout"),
        Some(5.0),
        "T#5s in seconds"
    );
    let interval = setting(&answer, "ServiceInterval");
    assert_eq!(
        (interval["unit"].clone(), interval["kind"].clone()),
        (json!("h"), json!("real"))
    );
    assert_eq!(number(&answer, "ServiceInterval"), Some(4000.0));
}

#[test]
fn an_operator_session_is_refused() {
    let running = start_plant("settings-operator");
    let anna = session_cookie(&running.base, "anna");
    let answer = read(&running, &anna);
    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("Engineer")),
        "{answer}"
    );
    let apply = settings(
        &running,
        &anna,
        json!({
            "faceplate": PANEL,
            "action": "apply",
            "changes": { "StartTimeout": 10 },
            "expected": { "StartTimeout": 5 },
        }),
    );
    assert_eq!(apply["ok"], json!(false), "{apply}");
    let erik = session_cookie(&running.base, "erik");
    assert_eq!(number(&read(&running, &erik), "StartTimeout"), Some(5.0));
}

#[test]
fn apply_writes_every_change_and_answers_with_the_values_read_back() {
    let running = start_plant("settings-apply");
    let erik = session_cookie(&running.base, "erik");
    let answer = settings(
        &running,
        &erik,
        json!({
            "faceplate": PANEL,
            "action": "apply",
            "changes": { "StartTimeout": 10, "ServiceInterval": 2000 },
            "expected": { "StartTimeout": 5, "ServiceInterval": 4000 },
        }),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(number(&answer, "StartTimeout"), Some(10.0));
    assert_eq!(number(&answer, "ServiceInterval"), Some(2000.0));
    let again = read(&running, &erik);
    assert_eq!(number(&again, "StartTimeout"), Some(10.0));
    assert_eq!(number(&again, "ServiceInterval"), Some(2000.0));
    // Settings are RETAIN variables of the block (Plant §10 item 1), so they survive a warm
    // restart and a power loss.
    let mut sources = SOURCES.to_vec();
    sources.push(PROGRAM);
    let declarations = CompileSession::from_sources(
        sources
            .iter()
            .map(|source| trust_runtime::harness::SourceFile::new(*source))
            .collect(),
    )
    .declarations()
    .expect("the Plant library compiles");
    let metadata = declarations.metadata();
    let block = metadata
        .function_blocks()
        .values()
        .find(|block| block.name.eq_ignore_ascii_case("Plant.FB_RunDevice"))
        .expect("the run device");
    for member in ["StartTimeout", "ServiceInterval"] {
        let declared = block
            .vars
            .iter()
            .find(|var| var.name.eq_ignore_ascii_case(member))
            .unwrap_or_else(|| panic!("{member} is declared"));
        assert_eq!(declared.retain, RetainPolicy::Retain, "{member}");
    }
}

#[test]
fn a_value_above_its_maximum_is_refused_naming_the_setting_and_nothing_is_written() {
    let running = start_plant("settings-limit");
    let erik = session_cookie(&running.base, "erik");
    let answer = settings(
        &running,
        &erik,
        json!({
            "faceplate": PANEL,
            "action": "apply",
            "changes": { "ServiceInterval": 2000, "StartTimeout": 700 },
            "expected": { "ServiceInterval": 4000, "StartTimeout": 5 },
        }),
    );
    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert_eq!(answer["field"], json!("StartTimeout"), "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("Start timeout") && error.contains("maximum")),
        "{answer}"
    );
    let again = read(&running, &erik);
    assert_eq!(number(&again, "StartTimeout"), Some(5.0));
    assert_eq!(
        number(&again, "ServiceInterval"),
        Some(4000.0),
        "all or none"
    );
}

#[test]
fn a_value_changed_meanwhile_is_refused_and_nothing_is_written() {
    let running = start_plant("settings-expected");
    let erik = session_cookie(&running.base, "erik");
    let answer = settings(
        &running,
        &erik,
        json!({
            "faceplate": PANEL,
            "action": "apply",
            "changes": { "StartTimeout": 10, "ServiceInterval": 2000 },
            "expected": { "StartTimeout": 7, "ServiceInterval": 4000 },
        }),
    );
    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert_eq!(answer["error"], json!("Changed meanwhile: refresh"));
    assert_eq!(answer["changed"], json!(true));
    let again = read(&running, &erik);
    assert_eq!(number(&again, "StartTimeout"), Some(5.0));
    assert_eq!(number(&again, "ServiceInterval"), Some(4000.0));
}

/// §21.4 item 4: the dialog sends each entry as its field holds it, and the runtime reads it. For
/// the pump, "soon" for the start timeout is no number: refused naming the setting and its field,
/// and the service interval of the same request is not written. Then "7,5" is 7.5 s (a comma is
/// the decimal separator) and " 2500 " is 2 500 h. Catches a text entry refused or written as
/// text, a comma not read as the decimal separator, a refusal without its field, and a refused
/// request that writes part of its changes.
#[test]
fn the_entries_as_the_dialog_holds_them_are_read_and_checked_by_the_runtime() {
    let running = start_plant("settings-entries");
    let erik = session_cookie(&running.base, "erik");
    let refused = settings(
        &running,
        &erik,
        json!({
            "faceplate": PANEL,
            "action": "apply",
            "changes": { "ServiceInterval": "2500", "StartTimeout": "soon" },
            "expected": { "ServiceInterval": 4000, "StartTimeout": 5 },
        }),
    );
    assert_eq!(refused["ok"], json!(false), "{refused}");
    assert_eq!(refused["field"], json!("StartTimeout"), "{refused}");
    assert_eq!(
        refused["error"],
        json!("Start timeout: enter a number, not \"soon\""),
        "{refused}"
    );
    assert_eq!(
        number(&read(&running, &erik), "ServiceInterval"),
        Some(4000.0),
        "all or none"
    );
    let answer = settings(
        &running,
        &erik,
        json!({
            "faceplate": PANEL,
            "action": "apply",
            "changes": { "StartTimeout": "7,5", "ServiceInterval": " 2500 " },
            "expected": { "StartTimeout": 5, "ServiceInterval": 4000 },
        }),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(number(&answer, "StartTimeout"), Some(7.5));
    assert_eq!(number(&answer, "ServiceInterval"), Some(2500.0));
}
