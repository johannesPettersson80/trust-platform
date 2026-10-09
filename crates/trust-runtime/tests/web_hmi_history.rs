//! Specification 39, §5.7 item 1: a trend's history from the runtime's historian
//! (`GET /api/hmi/history`).

use std::time::Duration;

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;
use trust_runtime::hmi::access::config::HmiAccessConfig;

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

const PROGRAM: &str = r#"
PROGRAM Main
VAR
    T : REAL;
    Lamp : BOOL;
END_VAR
    T := T + 1.0;
    Lamp := NOT Lamp;
END_PROGRAM
"#;

/// A page with a trend of `Main.T`, and a limit band that is no pen.
const TREND_PAGE: &str = r#"format = 1
title = "Trends"

[[object]]
id = "trend"
type = "trend"
at = [0, 0]
size = [600, 300]

[object.bind]
temperature = "Main.T"
high-limit = "Main.Lamp"
"#;

fn history(base: &str, query: &str) -> (u16, Value) {
    let (status, body, _) = http_request(
        "GET",
        base,
        &format!("/api/hmi/history?{query}"),
        None,
        None,
    );
    (status, body)
}

#[test]
fn a_trend_gets_its_pens_samples_of_the_span_from_the_historian() {
    let root = project("history", &[("hmi/screen.toml", SCREEN)]);
    let running = start_with_historian(&root, PROGRAM);
    std::thread::sleep(Duration::from_millis(1_200));
    let (status, body) = history(
        &running.base,
        "path=Main.T&path=Main.Lamp&path=Main.Gone&span=60",
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["enabled"], json!(true));
    let samples = body["samples"]["Main.T"]
        .as_array()
        .expect("Main.T's samples");
    assert!(samples.len() >= 3, "a sample each 100 ms: {body}");
    let pairs: Vec<(u64, f64)> = samples
        .iter()
        .map(|sample| {
            (
                sample[0].as_u64().expect("a time"),
                sample[1].as_f64().expect("a number"),
            )
        })
        .collect();
    assert!(
        pairs
            .windows(2)
            .all(|pair| pair[0].0 < pair[1].0 && pair[0].1 < pair[1].1),
        "oldest first: {pairs:?}"
    );
    let on = body["samples"]["Main.Lamp"]
        .as_array()
        .expect("Main.Lamp's samples");
    assert!(
        on.iter()
            .all(|sample| sample[1] == json!(0.0) || sample[1] == json!(1.0)),
        "booleans as 0 or 1: {body}"
    );
    assert_eq!(
        body["samples"]["Main.Gone"],
        json!([]),
        "a path the historian does not record"
    );
}

/// History is on by default (§5.9 item 1); without the historian a value no trend shows has none.
#[test]
fn without_the_historian_a_value_no_trend_shows_has_no_history() {
    let root = project("history-off", &[("hmi/screen.toml", SCREEN)]);
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        Default::default(),
        PROGRAM,
        None,
    );
    let (status, body) = history(&running.base, "path=Main.T");
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body,
        json!({ "enabled": true, "samples": { "Main.T": [] } })
    );
}

#[test]
fn a_history_request_names_1_to_16_paths_and_a_span_of_1_to_86400_seconds() {
    let root = project("history-limits", &[("hmi/screen.toml", SCREEN)]);
    let running = start_with_historian(&root, PROGRAM);
    let many = (0..17)
        .map(|index| format!("path=Main.P{index}"))
        .collect::<Vec<_>>()
        .join("&");
    for (query, words) in [
        ("span=60", "1 to 16 paths"),
        (many.as_str(), "1 to 16 paths"),
        ("path=Main.T&span=0", "1 to 86400 seconds"),
        ("path=Main.T&span=86401", "1 to 86400 seconds"),
        ("path=Main.T&span=soon", "1 to 86400 seconds"),
    ] {
        let (status, body) = history(&running.base, query);
        assert_eq!(status, 400, "{query}: {body}");
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|error| error.contains(words)),
            "{query}: {body}"
        );
    }
}

#[test]
fn the_history_needs_the_viewer_role() {
    let root = project("history-access", &[("hmi/screen.toml", SCREEN)]);
    let running = start(&root, WebAuthMode::Token, Some("secret"));
    let (status, _) = history(&running.base, "path=Main.P101.Speed");
    assert_eq!(status, 401);
}

/// Specification 39, §5.9 item 1: history is on by default. A value a trend of the active bundle
/// shows is kept from the program's calls of `Hmi.Display`, about once a second, with no historian
/// and no screen open; a value no trend shows (here a limit band) is not kept.
#[test]
fn a_trended_value_is_kept_without_a_historian_and_without_an_open_screen() {
    let root = project(
        "trend-history",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/trends.toml", TREND_PAGE),
        ],
    );
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        PROGRAM,
        None,
    );
    std::thread::sleep(Duration::from_millis(3_300));
    let (status, body) = history(&running.base, "path=Main.T&path=Main.Lamp&span=60");
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body["enabled"],
        json!(true),
        "history is on by default: {body}"
    );
    let samples = body["samples"]["Main.T"]
        .as_array()
        .expect("Main.T's samples");
    let times: Vec<u64> = samples
        .iter()
        .map(|sample| sample[0].as_u64().expect("a time"))
        .collect();
    assert!(times.len() >= 2, "a sample about each second: {body}");
    assert!(
        times
            .windows(2)
            .all(|pair| pair[1] / 1_000 > pair[0] / 1_000),
        "at most one sample a second, oldest first: {times:?}"
    );
    assert_eq!(
        body["samples"]["Main.Lamp"],
        json!([]),
        "a limit band is no pen: {body}"
    );
}

/// A panel whose trend shows a field, opened from an object for its instance.
const HEATER_PANEL: &str = r#"format = 1
title = "Heater"
size = [400, 300]

[fields.T]
accepts = ["number"]

[[object]]
id = "history"
type = "trend"
at = [0, 0]
size = [400, 300]

[object.bind]
temperature = "$T"
"#;

/// An equipment object that opens the heater panel for `Main.H1`.
const HEATER_PAGE: &str = r#"format = 1
title = "Overview"

[[object]]
id = "h1"
type = "pump"
block = "Heater"
instance = "Main.H1"
faceplate = "heater"
at = [0, 0]
size = [128, 108]
"#;

const HEATER_PROGRAM: &str = r#"
FUNCTION_BLOCK Heater
VAR_OUTPUT
    T : REAL;
END_VAR
    T := T + 1.0;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    H1 : Heater;
END_VAR
    H1();
END_PROGRAM
"#;

/// Specification 39, §5.9 item 1 and §8.7: a panel's trend is kept for the instance an object
/// opens it for, with no screen open, so the panel's trend opens filled.
#[test]
fn a_panels_trend_is_kept_for_the_instance_an_object_opens_it_for() {
    let root = project(
        "panel-trend-history",
        &[
            ("hmi/screen.toml", SCREEN),
            ("hmi/pages/overview.toml", HEATER_PAGE),
            ("hmi/panels/heater.toml", HEATER_PANEL),
        ],
    );
    let running = start_with(
        &root,
        WebAuthMode::Local,
        None,
        HmiAccessConfig::default(),
        HEATER_PROGRAM,
        None,
    );
    std::thread::sleep(Duration::from_millis(3_300));
    let (status, body) = history(&running.base, "path=Main.H1.T&span=60");
    assert_eq!(status, 200, "{body}");
    let samples = body["samples"]["Main.H1.T"]
        .as_array()
        .expect("Main.H1.T's samples");
    assert!(samples.len() >= 2, "a sample about each second: {body}");
}
