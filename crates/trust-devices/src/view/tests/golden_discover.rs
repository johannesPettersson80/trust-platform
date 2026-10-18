//! The golden Discover views: the JSON of each Discover pane scenario, which the webview's render
//! test `devices-discover-render.test.ts` reads (specification 25 §2). A run with
//! `TRUST_WRITE_DEVICES_DISCOVER_FIXTURES` set writes them to
//! `crates/trust-devices/tests/fixtures/devices_discover/`; every other run compares them byte for
//! byte. The files are never written by hand: change a scenario here and run the fixture tool
//! again.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::texts::discover as texts;
use crate::view::discover::{project, RuntimeOrigin, Scan};

/// The time of every view, in milliseconds since the Unix epoch.
const NOW_MS: u64 = 1_760_000_000_000;

/// The sources of a search, in the order of the search's own table (specification 11, "Sources").
const SOURCES: [&str; 10] = [
    "trust_runtime",
    "ads",
    "modbus_tcp",
    "mqtt",
    "opcua",
    "ethercat",
    "gpio",
    "serial",
    "usb",
    "bluetooth",
];

/// Set to write the fixtures instead of comparing them.
const WRITE: &str = "TRUST_WRITE_DEVICES_DISCOVER_FIXTURES";

/// The command that writes them.
const WRITE_COMMAND: &str =
    "TRUST_WRITE_DEVICES_DISCOVER_FIXTURES=1 cargo test -p trust-devices --lib golden_discover_matches_the_fixtures";

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/devices_discover")
}

/// The fixture text of a value: pretty JSON with a final newline.
fn text(value: &Value) -> String {
    let json = serde_json::to_string_pretty(value).expect("a view serializes to JSON");
    format!("{json}\n")
}

/// A search status with the `sources` and `devices` of its answer, as `comm.search.status` gives it.
fn status(search_id: &str, state: &str, sources: &Value, devices: &Value) -> Map<String, Value> {
    json!({
        "schema_version": 1,
        "search_id": search_id,
        "state": state,
        "sources": sources,
        "devices": devices,
    })
    .as_object()
    .cloned()
    .expect("a search status is a JSON object")
}

/// The Discover view of `answer`, projected for `scan` with the origins `runtimes`.
fn discover_view(answer: &Map<String, Value>, scan: &Scan, runtimes: &[RuntimeOrigin]) -> Value {
    let view =
        project(answer, scan, runtimes, NOW_MS).expect("the status reads as a search status");
    serde_json::to_value(view).expect("a Discover view serializes to JSON")
}

/// The origin `runtime:cell`, named Cell, which is ready (its hardware scans run) or stopped.
fn cell(ready: bool) -> RuntimeOrigin {
    RuntimeOrigin {
        id: "runtime:cell".to_owned(),
        name: "Cell".to_owned(),
        ready,
    }
}

/// A device with the results `finds`, seen three seconds before the view.
fn device(id: &str, addresses: &[&str], finds: &[Value]) -> Value {
    json!({
        "id": id,
        "addresses": addresses,
        "last_seen_ms": NOW_MS - 3000,
        "finds": finds,
    })
}

/// An ADS device of the AMS Net ID `192.168.1.20.1.1`, seen three seconds before the view.
fn ads_device() -> Value {
    json!({
        "id": "dev:netid:192.168.1.20.1.1",
        "addresses": ["192.168.1.20"],
        "ams_net_ids": ["192.168.1.20.1.1"],
        "last_seen_ms": NOW_MS - 3000,
        "finds": [ads_find("192.168.1.20.1.1")],
    })
}

/// A Modbus TCP result on `host`, found by a port that answers.
fn modbus_find(host: &str) -> Value {
    json!({
        "protocol": "modbus_tcp",
        "source": "tcp_connect",
        "confidence": "port_reachable",
        "simulated": false,
        "identity": { "kind": "modbus", "unit_id": 1 },
        "params": { "host": host, "port": 502, "unit_id": 1 },
        "warnings": [],
    })
}

/// An ADS result of the AMS Net ID `net_id`, confirmed by its identify answer.
fn ads_find(net_id: &str) -> Value {
    json!({
        "protocol": "ads",
        "source": "ads_identify",
        "confidence": "confirmed",
        "simulated": false,
        "identity": { "kind": "ads", "ams_net_id": net_id, "computer_name": "PLC-7" },
        "params": { "ams_net_id": net_id, "host": "192.168.1.20" },
        "warnings": [],
    })
}

/// A truST runtime found by mDNS, whose control endpoint is `endpoint`.
fn runtime_find(endpoint: &str) -> Value {
    json!({
        "protocol": "trust_runtime",
        "source": "mdns",
        "confidence": "confirmed",
        "simulated": false,
        "identity": {
            "kind": "trust_runtime",
            "name": "Line runtime",
            "id": "rt-line",
            "control": endpoint,
            "version": "0.24.75",
            "platform": "linux-x86_64",
        },
        "params": { "control_endpoint": endpoint, "name": "Line runtime", "host": "10.0.0.6" },
        "warnings": [],
    })
}

/// A search that runs: a truST runtime and a Modbus device are found, ADS is still searching, and
/// the origin is this computer. The two cards are a runtime with `Adopt` and a device with `+ Add`.
fn running_search() -> Value {
    let sources = json!([
        { "id": "trust_runtime", "state": "found", "count": 1 },
        { "id": "ads", "state": "searching", "count": 0 },
        { "id": "modbus_tcp", "state": "found", "count": 1 },
    ]);
    let devices = json!([
        device(
            "dev:ip:10.0.0.6",
            &["10.0.0.6"],
            &[runtime_find("tcp://10.0.0.6:5680")]
        ),
        device(
            "dev:ip:192.168.1.20",
            &["192.168.1.20"],
            &[modbus_find("192.168.1.20")]
        ),
    ]);
    discover_view(
        &status("s-1", "running", &sources, &devices),
        &Scan::ThisHost,
        &[cell(true)],
    )
}

/// A finished search with one ADS device: the truST runtimes found nothing, ADS found one device,
/// and `EtherCAT` was skipped with its reason.
fn finished_search_skipped_source() -> Value {
    let sources = json!([
        { "id": "trust_runtime", "state": "found", "count": 0 },
        { "id": "ads", "state": "found", "count": 1 },
        {
            "id": "ethercat",
            "state": "skipped",
            "reason": "EtherCAT is searched inside a runtime, on the computer with the bus adapter.",
        },
    ]);
    let devices = json!([ads_device()]);
    discover_view(
        &status("s-2", "done", &sources, &devices),
        &Scan::ThisHost,
        &[cell(true)],
    )
}

/// A finished search that found nothing: ADS timed out, and the origin is this computer.
fn empty_result() -> Value {
    let sources = json!([
        { "id": "trust_runtime", "state": "found", "count": 0 },
        { "id": "ads", "state": "timed_out", "count": 0 },
        { "id": "modbus_tcp", "state": "found", "count": 0 },
    ]);
    discover_view(
        &status("s-3", "done", &sources, &json!([])),
        &Scan::ThisHost,
        &[cell(true)],
    )
}

/// A stopped runtime is the origin of the search: every scan from it is disabled with its reason.
fn stopped_runtime_origin() -> Value {
    let sources = json!([
        { "id": "trust_runtime", "state": "found", "count": 0 },
        {
            "id": "gpio",
            "state": "skipped",
            "reason": "Start or connect a runtime before scanning EtherCAT or GPIO.",
        },
    ]);
    discover_view(
        &status("s-4", "done", &sources, &json!([])),
        &Scan::Runtime("runtime:cell".to_owned()),
        &[cell(false)],
    )
}

/// An older runtime cannot search: every source is skipped with the update it needs (specification
/// 34 TARGET-05). The status has no search ID, as the engine builds it.
fn older_runtime_update() -> Value {
    let reason = texts::update_needed("0.24.75");
    let sources: Vec<Value> = SOURCES
        .iter()
        .map(|id| json!({ "id": id, "state": "skipped", "reason": reason }))
        .collect();
    let mut answer = Map::new();
    answer.insert("state".to_owned(), json!("done"));
    answer.insert("sources".to_owned(), Value::Array(sources));
    answer.insert("devices".to_owned(), json!([]));
    let runtime = RuntimeOrigin {
        id: "runtime:old".to_owned(),
        name: "Old runtime".to_owned(),
        ready: true,
    };
    discover_view(
        &answer,
        &Scan::Runtime("runtime:old".to_owned()),
        &[runtime],
    )
}

/// Every scenario by its file name: one Discover view each, projected from a search status.
fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("running_search", running_search()),
        (
            "finished_search_skipped_source",
            finished_search_skipped_source(),
        ),
        ("empty_result", empty_result()),
        ("stopped_runtime_origin", stopped_runtime_origin()),
        ("older_runtime_update", older_runtime_update()),
    ]
}

#[test]
fn golden_discover_matches_the_fixtures() {
    let directory = directory();
    let scenarios = all();
    if std::env::var_os(WRITE).is_some() {
        fs::create_dir_all(&directory).expect("create the fixture folder");
        for (name, value) in &scenarios {
            fs::write(directory.join(format!("{name}.json")), text(value))
                .expect("write a fixture");
        }
        return;
    }
    let mut problems = Vec::new();
    for (name, value) in &scenarios {
        let path = directory.join(format!("{name}.json"));
        match fs::read_to_string(&path) {
            Ok(fixture) if fixture == text(value) => {}
            // The view as it is now is part of the message, so a reviewer sees what changed.
            Ok(_) => problems.push(format!(
                "{name}.json differs from the view, which is now:\n{}",
                text(value)
            )),
            Err(error) => problems.push(format!("{name}.json cannot be read: {error}")),
        }
    }
    let names: BTreeSet<String> = scenarios
        .iter()
        .map(|(name, _)| format!("{name}.json"))
        .collect();
    if let Ok(entries) = fs::read_dir(&directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file = entry.file_name().to_string_lossy().into_owned();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
                && !names.contains(&file)
            {
                problems.push(format!("{file} belongs to no scenario"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "the golden Discover views in {} do not match:\n  {}\nWrite them with `{WRITE_COMMAND}` and review the diff.",
        directory.display(),
        problems.join("\n  ")
    );
}

#[test]
fn golden_discover_scenarios_are_the_same_every_time() {
    for ((name, first), (_, second)) in all().iter().zip(all()) {
        assert_eq!(
            text(first),
            text(&second),
            "{name} is the same view every time"
        );
    }
}
