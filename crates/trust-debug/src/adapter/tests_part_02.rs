use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn dispatch_initialize_emits_initialized_event() {
    let mut adapter = DebugAdapter::new(DebugSession::new());
    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "initialize".to_string(),
        arguments: Some(serde_json::to_value(InitializeArguments::default()).unwrap()),
    };

    let outcome = adapter.dispatch_request(request);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<InitializeResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    let capabilities = response.body.unwrap().capabilities;
    assert_eq!(capabilities.supports_conditional_breakpoints, Some(true));
    assert_eq!(
        capabilities.supports_hit_conditional_breakpoints,
        Some(true)
    );
    assert_eq!(capabilities.supports_log_points, Some(true));
    let saw_initialized = outcome.events.iter().any(|value| {
        let event: Event<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
        event.event == "initialized"
    });
    assert!(saw_initialized);
    let internal_messages = outcome
        .events
        .iter()
        .filter_map(|value| {
            let event: Event<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
            if event.event != "trustDebugInternal" {
                return None;
            }
            event.body.and_then(|body| {
                body.get("message")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
        })
        .collect::<Vec<_>>();
    assert!(!internal_messages.is_empty());
    assert!(internal_messages
        .iter()
        .all(|message| message.contains("[trust-debug]")));
    let visible_internal_output = outcome.events.iter().any(|value| {
        let event: Event<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
        event.event == "output"
            && event.body.as_ref().is_some_and(|body| {
                body.get("output")
                    .and_then(|value| value.as_str())
                    .is_some_and(|output| output.contains("[trust-debug]"))
            })
    });
    assert!(
        !visible_internal_output,
        "internal debug traces must not be visible Debug Console output: {internal_messages:?}"
    );
}

#[test]
fn dispatch_launch_does_not_emit_initialized_event_without_initialize() {
    let mut adapter = DebugAdapter::new(DebugSession::new());

    let mut additional = BTreeMap::new();
    additional.insert(
        "program".to_string(),
        serde_json::Value::String("main.st".to_string()),
    );
    let launch_request = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "launch".to_string(),
        arguments: Some(serde_json::to_value(LaunchArguments { additional }).unwrap()),
    };

    let outcome = adapter.dispatch_request(launch_request);
    let saw_initialized = outcome.events.iter().any(|value| {
        let event: Event<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
        event.event == "initialized"
    });
    assert!(!saw_initialized);
}

#[test]
fn attach_set_expression_forwards_remote_io_force_and_release() {
    let (addr, requests, server) = spawn_remote_io_force_server();
    let mut adapter = DebugAdapter::new(DebugSession::new());
    adapter.remote_session = Some(
        super::super::remote::RemoteSession::connect(
            super::super::remote::RemoteEndpoint::Tcp(addr),
            Some("token".to_string()),
        )
        .expect("remote session should connect"),
    );

    let force = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setExpression".to_string(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "%QX0.0".to_string(),
                value: "force: TRUE".to_string(),
                frame_id: None,
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(force);
    assert_eq!(outcome.responses.len(), 1);
    assert_eq!(outcome.events.len(), 1);
    let response: Response<SetExpressionResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "remote force failed: {:?}",
        response.message
    );
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && entry.forced));

    let release = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "setExpression".to_string(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "%QX0.0".to_string(),
                value: "release".to_string(),
                frame_id: None,
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(release);
    assert_eq!(outcome.responses.len(), 1);
    assert_eq!(outcome.events.len(), 1);
    let response: Response<SetExpressionResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "remote release failed: {:?}",
        response.message
    );
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && !entry.forced));

    drop(adapter);
    server.join().expect("server should stop cleanly");
    let seen = requests.lock().expect("requests").clone();
    let types = seen
        .iter()
        .filter_map(|request| request.get("type").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    assert!(
        types.contains(&"io.force"),
        "setExpression force should call io.force: {types:?}"
    );
    assert!(
        types.contains(&"io.unforce"),
        "setExpression release should call io.unforce: {types:?}"
    );
}

#[test]
fn attach_st_io_force_and_release_forward_remote_io_force_and_release() {
    let (addr, requests, server) = spawn_remote_io_force_server();
    let mut adapter = DebugAdapter::new(DebugSession::new());
    adapter.remote_session = Some(
        super::super::remote::RemoteSession::connect(
            super::super::remote::RemoteEndpoint::Tcp(addr),
            Some("token".to_string()),
        )
        .expect("remote session should connect"),
    );

    let force = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stIoForce".to_string(),
        arguments: Some(
            serde_json::to_value(IoWriteArguments {
                address: "%QX0.0".to_string(),
                value: "TRUE".to_string(),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(force);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<serde_json::Value> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "stIoForce failed in attach mode: {:?}",
        response.message
    );
    assert_eq!(
        outcome.events.len(),
        0,
        "stIoForce must not emit an optimistic stale remote snapshot"
    );
    let state = Request::<serde_json::Value> {
        seq: 2,
        message_type: MessageType::Request,
        command: "stIoState".to_string(),
        arguments: None,
    };
    let outcome = adapter.dispatch_request(state);
    assert_eq!(outcome.events.len(), 1);
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && entry.forced));

    let release = Request {
        seq: 3,
        message_type: MessageType::Request,
        command: "stIoRelease".to_string(),
        arguments: Some(
            serde_json::to_value(IoReleaseArguments {
                address: "%QX0.0".to_string(),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(release);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<serde_json::Value> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "stIoRelease failed in attach mode: {:?}",
        response.message
    );
    assert_eq!(
        outcome.events.len(),
        0,
        "stIoRelease must not emit an optimistic stale remote snapshot"
    );
    let state = Request::<serde_json::Value> {
        seq: 4,
        message_type: MessageType::Request,
        command: "stIoState".to_string(),
        arguments: None,
    };
    let outcome = adapter.dispatch_request(state);
    assert_eq!(outcome.events.len(), 1);
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && !entry.forced));

    drop(adapter);
    server.join().expect("server should stop cleanly");
    let seen = requests.lock().expect("requests").clone();
    let types = seen
        .iter()
        .filter_map(|request| request.get("type").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    assert!(
        types.contains(&"io.force"),
        "stIoForce should call io.force: {types:?}"
    );
    assert!(
        types.contains(&"io.unforce"),
        "stIoRelease should call io.unforce: {types:?}"
    );
}

fn spawn_remote_io_force_server() -> (
    SocketAddr,
    Arc<Mutex<Vec<serde_json::Value>>>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let server_requests = Arc::clone(&requests);
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .expect("write timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut forced = false;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            let request: serde_json::Value =
                serde_json::from_str(&line).expect("control request json");
            server_requests
                .lock()
                .expect("requests lock")
                .push(request.clone());
            let id = request
                .get("id")
                .cloned()
                .unwrap_or_else(|| serde_json::json!(0));
            let response = match request.get("type").and_then(serde_json::Value::as_str) {
                Some("io.force") => {
                    forced = true;
                    serde_json::json!({"id": id, "ok": true, "result": {"status": "forced"}})
                }
                Some("io.unforce") => {
                    forced = false;
                    serde_json::json!({"id": id, "ok": true, "result": {"status": "released"}})
                }
                Some("io.read") => serde_json::json!({
                    "id": id,
                    "ok": true,
                    "result": {
                        "snapshot": {
                            "inputs": [],
                            "outputs": [{
                                "name": "OUT0",
                                "address": "%QX0.0",
                                "value": "Bool(false)",
                                "forced": forced
                            }],
                            "memory": []
                        }
                    }
                }),
                other => serde_json::json!({
                    "id": id,
                    "ok": false,
                    "error": format!("unexpected request {other:?}")
                }),
            };
            writeln!(stream, "{response}").expect("write control response");
        }
    });
    (addr, requests, handle)
}
