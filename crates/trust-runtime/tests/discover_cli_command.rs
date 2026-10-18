//! `trust-runtime discover`, the one search from the command line (specification 11, "One
//! search, device identity, and presence": "Command line").

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

fn run_discover(args: &[&str]) -> Output {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("discover")
    .args(args)
    .env_remove("TRUST_CTL_TOKEN")
    .output()
    .expect("run trust-runtime discover")
}

/// A reachable TCP port that speaks no protocol: it takes each connection, reads one byte and
/// closes it.
fn silent_port() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let address = listener.local_addr().expect("listener address").to_string();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let mut byte = [0_u8; 1];
            let _ = stream.read(&mut byte);
        }
    });
    address
}

/// A runtime that answers `comm.search.start` and then a finished `comm.search.status`, and
/// hands back each request it read.
fn fake_runtime() -> (String, Receiver<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake runtime");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("address"));
    let (requests, received) = mpsc::channel();
    thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept control client");
        let mut writer = stream.try_clone().expect("clone control stream");
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else {
                break;
            };
            let request: Value = serde_json::from_str(&line).expect("control request");
            let result = match request["type"].as_str() {
                Some("comm.search.start") => json!({ "search_id": "s-7" }),
                Some("comm.search.status") => json!({
                    "schema_version": 1, "search_id": "s-7", "origin": "runtime",
                    "state": "done", "deadline_ms": 10_000, "elapsed_ms": 1_200, "cursor": 1,
                    "sources": [{ "id": "ads", "state": "found", "count": 1 }],
                    "devices": [{
                        "id": "dev:ip:192.168.10.5", "addresses": ["192.168.10.5"],
                        "ams_net_ids": ["5.80.201.232.1.1"], "last_seen_ms": 1,
                        "finds": [{
                            "protocol": "ads", "source": "ads_identify", "confidence": "confirmed",
                            "simulated": false, "params": {}, "last_seen_ms": 1,
                            "identity": { "kind": "ads", "ams_net_id": "5.80.201.232.1.1" }
                        }]
                    }],
                    "warnings": []
                }),
                _ => json!({}),
            };
            let response = json!({ "id": request["id"], "ok": true, "result": result });
            let _ = requests.send(request);
            writeln!(writer, "{response}").expect("answer control request");
        }
    });
    (endpoint, received)
}

fn json_lines(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line)
                .unwrap_or_else(|error| panic!("not one JSON object per line: {line:?}: {error}"))
        })
        .collect()
}

fn events<'a>(lines: &'a [Value], event: &str) -> Vec<&'a Value> {
    lines.iter().filter(|line| line["event"] == event).collect()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn cli_discover_prints_one_json_object_per_line() {
    let address = silent_port();
    let output = run_discover(&["--json", "--only", "modbus_tcp", "--host", &address]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let lines = json_lines(&output);
    assert!(
        lines
            .iter()
            .all(|line| line.is_object() && line["event"].is_string()),
        "{lines:?}"
    );

    let devices = events(&lines, "device");
    assert_eq!(devices.len(), 1, "{lines:?}");
    assert_eq!(devices[0]["id"], json!("dev:ip:127.0.0.1"));
    assert_eq!(devices[0]["addresses"], json!(["127.0.0.1"]));
    let find = &devices[0]["finds"][0];
    assert_eq!(find["protocol"], json!("modbus_tcp"));
    assert_eq!(find["confidence"], json!("port_reachable"));
    assert_eq!(find["simulated"], json!(false));
    assert_eq!(find["params"]["address"], json!(address));

    let source = events(&lines, "source")
        .into_iter()
        .next_back()
        .expect("a source line");
    assert_eq!(source["id"], json!("modbus_tcp"));
    assert_eq!(source["state"], json!("found"));
    assert_eq!(source["count"], json!(1));

    let end = lines.last().expect("output lines");
    assert_eq!(end["event"], json!("end"));
    assert_eq!(end["state"], json!("done"));
    assert_eq!(end["origin"], json!("this_host"));
    assert_eq!(end["devices"], json!(1));
    assert_eq!(events(&lines, "end").len(), 1);
}

#[test]
fn cli_only_limits_the_sources() {
    let address = silent_port();
    let output = run_discover(&["--json", "--only", "modbus_tcp,mqtt", "--host", &address]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let lines = json_lines(&output);
    let sources: BTreeSet<&str> = events(&lines, "source")
        .into_iter()
        .filter_map(|line| line["id"].as_str())
        .collect();
    assert_eq!(sources, BTreeSet::from(["modbus_tcp", "mqtt"]));
    let device = events(&lines, "device")
        .into_iter()
        .next_back()
        .expect("a device line");
    let protocols: BTreeSet<&str> = device["finds"]
        .as_array()
        .expect("finds")
        .iter()
        .filter_map(|find| find["protocol"].as_str())
        .collect();
    assert_eq!(protocols, BTreeSet::from(["modbus_tcp", "mqtt"]));

    let unknown = run_discover(&["--json", "--only", "modbus"]);
    assert!(!unknown.status.success(), "an unknown source is refused");
    assert!(
        stderr(&unknown).contains("modbus_tcp"),
        "the refusal names the known sources: {}",
        stderr(&unknown)
    );
    assert!(unknown.stdout.is_empty());
}

#[test]
fn cli_discover_endpoint_runs_the_search_inside_the_runtime() {
    let (endpoint, requests) = fake_runtime();
    let output = run_discover(&[
        "--json",
        "--endpoint",
        &endpoint,
        "--token",
        "cli-token",
        "--only",
        "ads",
        "--host",
        "192.168.10.5",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let start = requests
        .recv_timeout(Duration::from_secs(5))
        .expect("comm.search.start");
    assert_eq!(start["type"], json!("comm.search.start"));
    assert_eq!(start["auth"], json!("cli-token"));
    assert_eq!(
        start["params"],
        json!({
            "only": ["ads"],
            "targets": { "hosts": ["192.168.10.5"] },
            "origin": "runtime"
        })
    );
    let status = requests
        .recv_timeout(Duration::from_secs(5))
        .expect("comm.search.status");
    assert_eq!(status["type"], json!("comm.search.status"));
    assert_eq!(status["params"], json!({ "search_id": "s-7" }));

    let lines = json_lines(&output);
    let devices = events(&lines, "device");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["id"], json!("dev:ip:192.168.10.5"));
    let end = lines.last().expect("output lines");
    assert_eq!(end["event"], json!("end"));
    assert_eq!(end["search_id"], json!("s-7"));
    assert_eq!(end["origin"], json!("runtime"));
    assert_eq!(end["devices"], json!(1));
}
