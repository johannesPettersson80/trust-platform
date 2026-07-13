use super::*;

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn simulator_status_is_false_until_successful_post_launch_actions_finish() {
    let project_root = simulator_status_project("ready");
    let mut adapter = DebugAdapter::new(DebugSession::new(Runtime::new()));

    assert_simulator_status(&mut adapter, false, false, false);

    let launch = simulator_launch_request(&project_root, "tcp://127.0.0.1:0", 10);
    let launch_outcome = adapter.dispatch_request(launch);
    assert!(
        launch_outcome.responses.is_empty(),
        "launch should remain deferred until configurationDone"
    );
    assert_simulator_status(&mut adapter, false, false, false);
    assert!(
        adapter
            .maybe_force_start_after_timeout("trustSimulatorStatus")
            .is_none(),
        "a readiness probe must not force a deferred launch"
    );
    assert!(adapter.launch_state.has_pending_launch());

    let configured = adapter.dispatch_request(configuration_done_request(11));
    assert!(
        configured.responses.iter().all(|value| {
            serde_json::from_value::<Response<serde_json::Value>>(value.clone())
                .is_ok_and(|response| response.success)
        }),
        "configurationDone and launch should succeed: {:?}",
        configured.responses
    );
    assert_simulator_status(&mut adapter, false, false, true);

    let actions = adapter.launch_state.take_actions();
    assert!(actions.start_runner_after_launch);
    adapter.start_runner();
    assert_simulator_status(&mut adapter, true, true, true);

    adapter.stop_runner();
    assert_simulator_status(&mut adapter, false, false, true);
    fs::remove_dir_all(project_root).ok();
}

#[test]
fn simulator_status_stays_false_after_launch_failure() {
    let project_root = simulator_status_project("failed");
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("tcp://{}", occupied.local_addr().unwrap());
    let mut adapter = DebugAdapter::new(DebugSession::new(Runtime::new()));

    let launch = simulator_launch_request(&project_root, &endpoint, 20);
    assert!(adapter.dispatch_request(launch).responses.is_empty());
    let configured = adapter.dispatch_request(configuration_done_request(21));
    assert!(configured.responses.iter().any(|value| {
        serde_json::from_value::<Response<serde_json::Value>>(value.clone())
            .is_ok_and(|response| !response.success)
    }));

    assert_simulator_status(&mut adapter, false, false, false);

    drop(occupied);
    fs::remove_dir_all(project_root).ok();
}
fn assert_simulator_status(
    adapter: &mut DebugAdapter,
    ready: bool,
    runner: bool,
    control_server: bool,
) {
    let outcome = adapter.dispatch_request(Request::<serde_json::Value> {
        seq: 100,
        message_type: MessageType::Request,
        command: "trustSimulatorStatus".to_string(),
        arguments: None,
    });
    let response: Response<serde_json::Value> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success, "status request should be supported");
    let body = response.body.expect("status response should have a body");
    assert_eq!(
        body.get("ready").and_then(serde_json::Value::as_bool),
        Some(ready)
    );
    assert_eq!(
        body.get("runner").and_then(serde_json::Value::as_bool),
        Some(runner)
    );
    assert_eq!(
        body.get("controlServer")
            .and_then(serde_json::Value::as_bool),
        Some(control_server)
    );
}

fn simulator_launch_request(
    project_root: &Path,
    control_endpoint: &str,
    seq: u32,
) -> Request<serde_json::Value> {
    let mut additional = BTreeMap::new();
    additional.insert(
        "program".to_string(),
        serde_json::Value::String(project_root.join("main.st").display().to_string()),
    );
    additional.insert(
        "runtimeRoot".to_string(),
        serde_json::Value::String(project_root.display().to_string()),
    );
    additional.insert(
        "controlEndpoint".to_string(),
        serde_json::Value::String(control_endpoint.to_string()),
    );
    additional.insert(
        "controlAuthToken".to_string(),
        serde_json::Value::String("simulator-status-test-token".to_string()),
    );
    Request {
        seq,
        message_type: MessageType::Request,
        command: "launch".to_string(),
        arguments: Some(serde_json::to_value(LaunchArguments { additional }).unwrap()),
    }
}

fn configuration_done_request(seq: u32) -> Request<serde_json::Value> {
    Request {
        seq,
        message_type: MessageType::Request,
        command: "configurationDone".to_string(),
        arguments: None,
    }
}

fn simulator_status_project(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-debug-simulator-status-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("main.st"), "PROGRAM Main\nEND_PROGRAM\n").unwrap();
    root
}
