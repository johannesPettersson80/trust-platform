//! Specification 39, §11 item 6: Live mode reads the bound paths of the page being edited with
//! the control request `hmi.read`, which answers as the runtime display's stream does.

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_runtime::config::WebAuthMode;

#[allow(dead_code)]
#[path = "web_hmi_display/support.rs"]
mod support;
use support::*;

fn hmi_read(base: &str, params: Value) -> Value {
    let mut response = ureq::post(&format!("{base}/api/control"))
        .config()
        .http_status_as_error(false)
        .build()
        .header("Content-Type", "application/json")
        .send(json!({ "id": 7, "type": "hmi.read", "params": params }).to_string())
        .expect("the control request");
    let status = response.status().as_u16();
    let body: Value =
        serde_json::from_str(&response.body_mut().read_to_string().unwrap_or_default())
            .unwrap_or(Value::Null);
    assert_eq!(status, 200, "{body}");
    body
}

#[test]
fn hmi_read_answers_the_values_the_cycle_and_the_missing_paths() {
    let root = hmi_project("live-read");
    let running = start(&root, WebAuthMode::Local, None);
    let paths = json!({ "paths": ["Main.P101.Speed", "Main.P101.Running", "Main.Gone"] });
    let deadline = Instant::now() + Duration::from_secs(3);
    let answer = loop {
        let answer = hmi_read(&running.base, paths.clone());
        if answer["result"]["values"]["Main.P101.Running"]["value"] == json!(true)
            || Instant::now() > deadline
        {
            break answer;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(answer["ok"], json!(true), "{answer}");
    let result = &answer["result"];
    assert_eq!(
        result["values"]["Main.P101.Speed"],
        json!({ "value": 1460.0 }),
        "{answer}"
    );
    assert_eq!(result["values"]["Main.Gone"], Value::Null);
    assert_eq!(result["missing"], json!(["Main.Gone"]));
    assert!(
        result["cycle"].as_u64().is_some_and(|cycle| cycle >= 1),
        "{answer}"
    );
}

#[test]
fn hmi_read_takes_at_most_1000_paths_each_a_path() {
    let root = hmi_project("live-read-limits");
    let running = start(&root, WebAuthMode::Local, None);
    let many: Vec<String> = (0..1001).map(|index| format!("Main.V{index}")).collect();
    let answer = hmi_read(&running.base, json!({ "paths": many }));
    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("at most 1000 paths")),
        "{answer}"
    );
    let answer = hmi_read(&running.base, json!({ "paths": ["Main..Speed"] }));
    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("Main..Speed")),
        "{answer}"
    );
}
