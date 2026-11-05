use serde_json::{json, Value};

use super::*;

const VERSION: &str = "0123456789abcdef0123456789abcdef";

fn gate(args: Value) -> Value {
    live_gate_value(vec![args])
}

/// The project's folder, and an unsaved document of its HMI.
const ROOT: &str = "/work/plant";
const UNSAVED_PAGE: &str = "/work/plant/hmi/pages/overview.toml";

/// A saved HMI with a checked bundle on a run target with an HMI web server, opening `overview`.
fn facts(changes: Value) -> Value {
    let mut facts = json!({
        "root": ROOT,
        "unsaved": [],
        "bundle": { "ok": true, "version": VERSION },
        "webServer": true,
        "page": "overview",
    });
    for (key, value) in changes.as_object().expect("changes are an object") {
        facts[key] = value.clone();
    }
    facts
}

fn served(status: u16, bundle: &str, page: &str) -> Value {
    json!({ "status": status, "body": { "bundle": bundle, "page": { "name": page } } })
}

fn reason(answer: &Value) -> &str {
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["verdict"]["kind"], "read_only", "{answer}");
    answer["verdict"]["reason"]
        .as_str()
        .expect("a read-only verdict has a reason")
}

// Catches a gate that compares the whole version, or another length of it, with the bundle the
// web server names: the web server names its active bundle by the version's first 16 characters,
// all of a shorter version.
#[test]
fn a_saved_checked_hmi_reads_its_page_then_operates_the_deployed_bundle() {
    assert_eq!(
        gate(facts(json!({}))),
        json!({ "ok": true, "verdict": { "kind": "read_page" } })
    );
    assert_eq!(
        gate(facts(
            json!({ "served": served(200, "0123456789abcdef", "overview") })
        )),
        json!({ "ok": true, "verdict": { "kind": "operate", "bundle": "0123456789abcdef" } })
    );
    assert_eq!(
        gate(facts(json!({
            "bundle": { "ok": true, "version": "v1" },
            "served": served(200, "v1", "overview"),
        }))),
        json!({ "ok": true, "verdict": { "kind": "operate", "bundle": "v1" } })
    );
}

// Catches the conditions checked in another order, and the simulator told to enable
// `[runtime.web]`, which only a runtime has (specification 39, §11 item 6).
#[test]
fn before_the_page_is_read_the_first_unmet_condition_is_the_reason() {
    let nothing_holds =
        json!({ "unsaved": [UNSAVED_PAGE], "bundle": { "ok": false }, "webServer": false });
    assert_eq!(
        reason(&gate(facts(nothing_holds))),
        "Save the HMI before operating its deployed version."
    );
    for bundle in [
        json!({ "ok": false, "version": VERSION }),
        json!({ "ok": true }),
        json!({ "ok": true, "version": "" }),
        Value::Null,
    ] {
        assert_eq!(
            reason(&gate(facts(
                json!({ "bundle": bundle, "webServer": false })
            ))),
            "Check and deploy the HMI before operating it.",
            "{bundle}"
        );
    }
    let without_web_server = |run_target: Value| {
        reason(&gate(facts(
            json!({ "webServer": false, "runTarget": run_target }),
        )))
        .to_owned()
    };
    assert_eq!(
        without_web_server(json!({ "label": "Simulator", "simulator": true })),
        "The simulator has no HMI web server. Operate the page, with its alarms, on a runtime: \
         select or connect one in Devices & Connections."
    );
    assert_eq!(
        without_web_server(json!({ "label": "Cell runtime", "simulator": false })),
        "Cell runtime has no HMI web server. Enable [runtime.web] in its runtime.toml."
    );
    assert_eq!(
        without_web_server(Value::Null),
        "The selected target has no HMI web server. Start a runtime with [runtime.web] enabled."
    );
}

// Catches operating a page that the web server serves from another deployment, or not at all,
// and an edit made while the page was read going unnoticed.
#[test]
fn a_page_not_served_from_the_project_bundle_stays_read_only() {
    let undeployed = "Deploy this saved HMI to the selected runtime before operating it.";
    for served in [
        served(404, "0123456789abcdef", "overview"),
        served(200, "0123456789abcdef", "alarms"),
        served(200, "deployed", "overview"),
        served(200, VERSION, "overview"),
        json!({ "status": 200, "body": { "bundle": "0123456789abcdef" } }),
        json!({ "status": 200 }),
    ] {
        assert_eq!(
            reason(&gate(facts(json!({ "served": served })))),
            undeployed,
            "{served}"
        );
    }
    assert_eq!(
        reason(&gate(facts(json!({
            "unsaved": [UNSAVED_PAGE],
            "served": served(200, "0123456789abcdef", "overview"),
        })))),
        "The HMI changed. Save and deploy it before operating again."
    );
    assert_eq!(
        reason(&gate(facts(json!({
            "unsaved": [UNSAVED_PAGE],
            "served": served(200, "deployed", "overview"),
        })))),
        undeployed,
        "a page of another deployment is named first"
    );
}

#[test]
fn arguments_it_cannot_read_answer_an_error() {
    for args in [
        json!({ "root": ROOT, "unsaved": [], "webServer": true }),
        facts(json!({ "dirty": false })),
        facts(json!({ "extra": 1 })),
        json!("open"),
    ] {
        let answer = gate(args.clone());
        assert_eq!(answer["ok"], false, "{args}");
        assert!(
            answer["error"]
                .as_str()
                .is_some_and(|error| error.starts_with("invalid trust-lsp.hmiLiveGate arguments")),
            "{answer}"
        );
    }
}
