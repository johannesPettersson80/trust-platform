use super::*;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn dispatch_set_expression_force_supports_output_and_memory_io() {
    let mut runtime = Runtime::new();
    let output_addr = IoAddress::parse("%QX0.0").unwrap();
    let memory_addr = IoAddress::parse("%MX0.0").unwrap();
    runtime.io_mut().bind("OUT0", output_addr.clone());
    runtime.io_mut().bind("MEM0", memory_addr.clone());

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let force_output = Request {
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
    let outcome = adapter.dispatch_request(force_output);
    assert_eq!(outcome.responses.len(), 1);
    assert_eq!(outcome.events.len(), 1);
    let response: Response<SetExpressionResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "force output failed: {:?}",
        response.message
    );
    let output_event: Event<IoStateEventBody> =
        serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(output_event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && entry.forced));

    let force_memory = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "setExpression".to_string(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "%MX0.0".to_string(),
                value: "force: TRUE".to_string(),
                frame_id: None,
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(force_memory);
    assert_eq!(outcome.responses.len(), 1);
    assert_eq!(outcome.events.len(), 1);
    let response: Response<SetExpressionResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "force memory failed: {:?}",
        response.message
    );
    let memory_event: Event<IoStateEventBody> =
        serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(memory_event
        .body
        .unwrap()
        .memory
        .iter()
        .any(|entry| entry.address == "%MX0.0" && entry.forced));

    let release_output = Request {
        seq: 3,
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
    let outcome = adapter.dispatch_request(release_output);
    assert_eq!(outcome.responses.len(), 1);
    assert_eq!(outcome.events.len(), 1);
    let release_event: Event<IoStateEventBody> =
        serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(release_event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && !entry.forced));

    let runtime = adapter.session().runtime_handle();
    let runtime = runtime.lock().unwrap();
    assert_eq!(
        runtime.io().read(&output_addr).unwrap(),
        RuntimeValue::Bool(true)
    );
    assert_eq!(
        runtime.io().read(&memory_addr).unwrap(),
        RuntimeValue::Bool(true)
    );
}

#[test]
fn dispatch_set_expression_write_once_rejects_output_io() {
    let mut runtime = Runtime::new();
    let output_addr = IoAddress::parse("%QX0.1").unwrap();
    runtime.io_mut().bind("OUT1", output_addr);

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setExpression".to_string(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "%QX0.1".to_string(),
                value: "TRUE".to_string(),
                frame_id: None,
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(request);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<serde_json::Value> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(!response.success);
    assert_eq!(
        response.message.as_deref(),
        Some("only input addresses can be written once")
    );
}

#[test]
fn attach_set_expression_forwards_remote_io_force_and_release() {
    let (addr, requests, server) = spawn_remote_io_force_server();
    let runtime = Runtime::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
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
    let runtime = Runtime::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
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
    assert_eq!(outcome.events.len(), 1);
    let response: Response<serde_json::Value> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "stIoForce failed in attach mode: {:?}",
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
    assert_eq!(outcome.events.len(), 1);
    let response: Response<serde_json::Value> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "stIoRelease failed in attach mode: {:?}",
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
        "stIoForce should call io.force: {types:?}"
    );
    assert!(
        types.contains(&"io.unforce"),
        "stIoRelease should call io.unforce: {types:?}"
    );
}

#[test]
fn dispatch_set_expression_force_supports_direct_instance_field_live() {
    let mut runtime = Runtime::new();
    let instance_id = runtime.storage_mut().create_instance("MAIN_T");
    runtime
        .storage_mut()
        .set_instance_var(instance_id, "count", RuntimeValue::Int(1));
    runtime
        .storage_mut()
        .set_global("Main", RuntimeValue::Instance(instance_id));

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setExpression".to_string(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "Main.count".to_string(),
                value: "force: 5".to_string(),
                frame_id: None,
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(request);
    let response: Response<SetExpressionResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "live direct instance-field force failed: {:?}",
        response.message
    );

    let runtime = adapter.session().runtime_handle();
    let runtime = runtime.lock().unwrap();
    assert_eq!(
        runtime.storage().get_instance_var(instance_id, "count"),
        Some(&RuntimeValue::Int(5))
    );
}

#[test]
fn dispatch_set_expression_force_supports_direct_instance_field_paused() {
    let mut runtime = Runtime::new();
    let instance_id = runtime.storage_mut().create_instance("MAIN_T");
    runtime
        .storage_mut()
        .set_instance_var(instance_id, "count", RuntimeValue::Int(1));
    runtime
        .storage_mut()
        .set_global("Main", RuntimeValue::Instance(instance_id));

    let control = DebugControl::new();
    control.refresh_snapshot_from_storage(runtime.storage(), runtime.current_time());

    let session = DebugSession::with_control(runtime, control);
    let mut adapter = DebugAdapter::new(session);

    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setExpression".to_string(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "Main.count".to_string(),
                value: "force: 5".to_string(),
                frame_id: None,
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(request);
    let response: Response<SetExpressionResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "paused direct instance-field force failed: {:?}",
        response.message
    );

    let snapshot_value = adapter.session().debug_control().with_snapshot(|snapshot| {
        snapshot
            .storage
            .get_instance_var(instance_id, "count")
            .cloned()
    });
    assert_eq!(snapshot_value, Some(Some(RuntimeValue::Int(5))));
}

#[test]
fn dispatch_initialize_emits_initialized_event() {
    let runtime = Runtime::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
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
    let runtime = Runtime::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));

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
fn debug_control_server_uses_launch_project_root_for_comm_apply() {
    let project_root = unique_project_root("debug-control-project-root");
    fs::write(project_root.join("main.st"), "PROGRAM Main\nEND_PROGRAM\n").unwrap();
    let addr = reserve_loopback_addr();
    let endpoint = format!("tcp://{addr}");
    let auth = "debug-control-token";

    let runtime = Runtime::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    launch_debug_control_session(&mut adapter, &project_root, &endpoint, auth);

    let schema = send_control_request(
        addr,
        auth,
        serde_json::json!({
            "id": 1,
            "type": "comm.schema",
            "params": { "protocol": "modbus_tcp" }
        }),
    );
    assert_control_ok(&schema, "comm.schema");

    let topology = send_control_request(
        addr,
        auth,
        serde_json::json!({
            "id": 2,
            "type": "fleet.topology"
        }),
    );
    assert_control_ok(&topology, "fleet.topology");

    let create = send_control_request(
        addr,
        auth,
        serde_json::json!({
            "id": 3,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "action": "add",
                "params": {
                    "address": "127.0.0.1:1502",
                    "unit_id": 2,
                    "input_start": 5,
                    "output_start": 7,
                    "timeout_ms": 750,
                    "on_error": "warn"
                }
            }
        }),
    );
    assert_control_ok(&create, "comm.apply create");
    assert_eq!(
        control_result(&create)
            .get("lifecycle_effect")
            .and_then(serde_json::Value::as_str),
        Some("restart_required")
    );
    assert_eq!(
        control_result(&create)
            .get("applied")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );

    let io_toml = project_root.join("io.toml");
    let created_text = fs::read_to_string(&io_toml).expect("debug control should create io.toml");
    assert!(
        created_text.contains("127.0.0.1:1502"),
        "created io.toml should contain first Modbus driver: {created_text}"
    );

    let update = send_control_request(
        addr,
        auth,
        serde_json::json!({
            "id": 4,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "action": "add",
                "params": {
                    "address": "127.0.0.1:1503",
                    "unit_id": 3,
                    "input_start": 9,
                    "output_start": 11,
                    "timeout_ms": 900,
                    "on_error": "warn"
                }
            }
        }),
    );
    assert_control_ok(&update, "comm.apply update");
    assert_eq!(
        control_result(&update)
            .get("lifecycle_effect")
            .and_then(serde_json::Value::as_str),
        Some("restart_required")
    );

    let updated_text = fs::read_to_string(&io_toml).expect("debug control should update io.toml");
    assert!(
        updated_text.contains("127.0.0.1:1502"),
        "update should preserve existing driver: {updated_text}"
    );
    assert!(
        updated_text.contains("127.0.0.1:1503"),
        "update should append second driver: {updated_text}"
    );
    assert!(
        updated_text.matches("modbus-tcp").count() >= 2,
        "update should persist multiple Modbus instances: {updated_text}"
    );

    fs::remove_dir_all(&project_root).ok();
}

#[test]
fn launch_fails_when_control_server_endpoint_is_already_in_use() {
    let project_root = unique_project_root("debug-control-endpoint-busy");
    fs::write(project_root.join("main.st"), "PROGRAM Main\nEND_PROGRAM\n").unwrap();
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("tcp://{}", occupied.local_addr().unwrap());

    let runtime = Runtime::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
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
        serde_json::Value::String(endpoint),
    );
    additional.insert(
        "controlAuthToken".to_string(),
        serde_json::Value::String("debug-control-token".to_string()),
    );

    let launch = Request {
        seq: 20,
        message_type: MessageType::Request,
        command: "launch".to_string(),
        arguments: Some(serde_json::to_value(LaunchArguments { additional }).unwrap()),
    };
    let launch_outcome = adapter.dispatch_request(launch);
    assert!(
        launch_outcome.responses.is_empty(),
        "launch should defer until configurationDone"
    );

    let configuration_done = Request {
        seq: 21,
        message_type: MessageType::Request,
        command: "configurationDone".to_string(),
        arguments: None,
    };
    let configured = adapter.dispatch_request(configuration_done);
    let failed = configured.responses.iter().any(|value| {
        let response: Response<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
        !response.success
            && response
                .message
                .as_deref()
                .is_some_and(|message| message.contains("control server start failed"))
    });
    assert!(
        failed,
        "control endpoint collision must fail launch, not fake a running runtime: {:?}",
        configured.responses
    );
    let scheduled_runner = configured.events.iter().any(|value| {
        let event: Event<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
        event.body.as_ref().is_some_and(|body| {
            body.get("message")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|message| message.contains("runner start scheduled"))
        })
    });
    assert!(
        !scheduled_runner,
        "failed control endpoint must not schedule the debug runner"
    );

    drop(occupied);
    fs::remove_dir_all(&project_root).ok();
}

#[test]
fn debug_control_server_serves_hmi_schema_and_values() {
    let project_root = unique_project_root("debug-control-hmi");
    fs::write(
        project_root.join("main.st"),
        r"
PROGRAM Main
VAR
    speed : REAL := 1.5;
    running : BOOL := TRUE;
END_VAR
END_PROGRAM
",
    )
    .unwrap();
    let addr = reserve_loopback_addr();
    let endpoint = format!("tcp://{addr}");
    let auth = "debug-control-hmi-token";

    let runtime = Runtime::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    launch_debug_control_session(&mut adapter, &project_root, &endpoint, auth);

    {
        let runtime = adapter.session().runtime_handle();
        let runtime = runtime.lock().unwrap();
        adapter
            .session()
            .debug_control()
            .refresh_snapshot_from_storage(runtime.storage(), runtime.current_time());
    }

    let schema = send_control_request(
        addr,
        auth,
        serde_json::json!({
            "id": 5,
            "type": "hmi.schema.get"
        }),
    );
    assert_control_ok(&schema, "hmi.schema.get");
    let widgets = control_result(&schema)
        .get("widgets")
        .and_then(serde_json::Value::as_array)
        .expect("hmi schema widgets");
    let speed_id = widgets
        .iter()
        .find(|widget| widget.get("path").and_then(serde_json::Value::as_str) == Some("Main.speed"))
        .and_then(|widget| widget.get("id"))
        .and_then(serde_json::Value::as_str)
        .expect("Main.speed widget should be in HMI schema")
        .to_string();

    let values = send_control_request(
        addr,
        auth,
        serde_json::json!({
            "id": 6,
            "type": "hmi.values.get",
            "params": { "ids": [speed_id] }
        }),
    );
    assert_control_ok(&values, "hmi.values.get");
    assert_eq!(
        control_result(&values)
            .get("connected")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "hmi.values.get should be connected to the debug snapshot: {values}"
    );
    let value_record = control_result(&values)
        .get("values")
        .and_then(|entries| entries.get(&speed_id))
        .expect("Main.speed value record");
    assert_eq!(
        value_record.get("q").and_then(serde_json::Value::as_str),
        Some("good"),
        "HMI value should come from the debug snapshot: {values}"
    );
    assert_eq!(
        value_record.get("v").and_then(serde_json::Value::as_f64),
        Some(1.5),
        "HMI value should preserve the runtime value: {values}"
    );

    fs::remove_dir_all(&project_root).ok();
}

fn unique_project_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-debug-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn reserve_loopback_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    addr
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

fn launch_debug_control_session(
    adapter: &mut DebugAdapter,
    project_root: &Path,
    endpoint: &str,
    auth: &str,
) {
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
        serde_json::Value::String(endpoint.to_string()),
    );
    additional.insert(
        "controlAuthToken".to_string(),
        serde_json::Value::String(auth.to_string()),
    );

    let launch = Request {
        seq: 10,
        message_type: MessageType::Request,
        command: "launch".to_string(),
        arguments: Some(serde_json::to_value(LaunchArguments { additional }).unwrap()),
    };
    let launch_outcome = adapter.dispatch_request(launch);
    assert!(
        launch_outcome.responses.is_empty(),
        "launch should defer until configurationDone"
    );

    let configuration_done = Request {
        seq: 11,
        message_type: MessageType::Request,
        command: "configurationDone".to_string(),
        arguments: None,
    };
    let configured = adapter.dispatch_request(configuration_done);
    assert!(
        configured.responses.iter().all(|value| {
            let response: Response<serde_json::Value> =
                serde_json::from_value(value.clone()).unwrap();
            response.success
        }),
        "configurationDone/launch responses should succeed: {:?}",
        configured.responses
    );
}

fn send_control_request(
    addr: SocketAddr,
    auth: &str,
    mut request: serde_json::Value,
) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_error = None;
    loop {
        match TcpStream::connect(addr) {
            Ok(mut stream) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                request
                    .as_object_mut()
                    .expect("control request should be an object")
                    .insert("auth".to_string(), serde_json::json!(auth));
                writeln!(stream, "{request}").unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                return serde_json::from_str(&line).expect("control response should be json");
            }
            Err(error) if Instant::now() < deadline => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!(
                "control endpoint {addr} did not accept connections: last={last_error:?}, final={error}"
            ),
        }
    }
}

fn assert_control_ok(response: &serde_json::Value, label: &str) {
    assert_eq!(
        response.get("ok").and_then(serde_json::Value::as_bool),
        Some(true),
        "{label} failed: {response}"
    );
}

fn control_result(response: &serde_json::Value) -> &serde_json::Value {
    response.get("result").expect("control response result")
}

#[test]
fn dispatch_run_controls_update_debug_mode() {
    let runtime = Runtime::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    let control = adapter.session().debug_control();

    let pause_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "pause".to_string(),
        arguments: Some(serde_json::to_value(PauseArguments { thread_id: 1 }).unwrap()),
    };
    adapter.dispatch_request(pause_req);
    assert_eq!(control.mode(), trust_runtime::debug::DebugMode::Paused);

    let step_in_req = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "stepIn".to_string(),
        arguments: Some(serde_json::to_value(StepInArguments { thread_id: 1 }).unwrap()),
    };
    adapter.dispatch_request(step_in_req);
    assert_eq!(control.mode(), trust_runtime::debug::DebugMode::Running);

    control.pause();
    let next_req = Request {
        seq: 3,
        message_type: MessageType::Request,
        command: "next".to_string(),
        arguments: Some(serde_json::to_value(NextArguments { thread_id: 1 }).unwrap()),
    };
    adapter.dispatch_request(next_req);
    assert_eq!(control.mode(), trust_runtime::debug::DebugMode::Running);

    control.pause();
    let step_out_req = Request {
        seq: 4,
        message_type: MessageType::Request,
        command: "stepOut".to_string(),
        arguments: Some(serde_json::to_value(StepOutArguments { thread_id: 1 }).unwrap()),
    };
    adapter.dispatch_request(step_out_req);
    assert_eq!(control.mode(), trust_runtime::debug::DebugMode::Running);

    control.pause();
    let continue_req = Request {
        seq: 5,
        message_type: MessageType::Request,
        command: "continue".to_string(),
        arguments: Some(serde_json::to_value(ContinueArguments { thread_id: 1 }).unwrap()),
    };
    adapter.dispatch_request(continue_req);
    assert_eq!(control.mode(), trust_runtime::debug::DebugMode::Running);
}
