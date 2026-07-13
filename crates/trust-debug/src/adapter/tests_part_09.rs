use super::*;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn launch_io_state_includes_configured_source_provenance() {
    let project_root = unique_project_root("launch-io-source");
    let src = project_root.join("src");
    fs::create_dir_all(&src).unwrap();
    let main_path = src.join("Main.st");
    let config_path = src.join("Config.st");
    fs::write(
        &main_path,
        r#"PROGRAM MainProgram
VAR
    e_stop : BOOL;
    running : BOOL;
END_VAR

running := e_stop;
END_PROGRAM
"#,
    )
    .unwrap();
    fs::write(
        &config_path,
        r#"CONFIGURATION Config
TASK Cycle (INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM P1 WITH Cycle : MainProgram;
VAR_CONFIG
    P1.e_stop AT %IX0.0 : BOOL;
    P1.running AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
"#,
    )
    .unwrap();
    fs::write(
        project_root.join("io.toml"),
        r#"[io]

[[io.drivers]]
name = "modbus-tcp"
params = { address = "127.0.0.1:1502", unit_id = 1, input_start = 0, output_start = 1, timeout_ms = 500, on_error = "fault" }
"#,
    )
    .unwrap();

    let mut adapter = DebugAdapter::new(DebugSession::new(Runtime::new()));
    let mut additional = BTreeMap::new();
    additional.insert(
        "program".to_string(),
        serde_json::Value::String(config_path.display().to_string()),
    );
    additional.insert(
        "runtimeRoot".to_string(),
        serde_json::Value::String(project_root.display().to_string()),
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

    let configuration_done = Request::<serde_json::Value> {
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

    let io_state = configured
        .events
        .iter()
        .filter_map(|value| serde_json::from_value::<Event<IoStateEventBody>>(value.clone()).ok())
        .find(|event| event.event == "stIoState")
        .and_then(|event| event.body)
        .expect("launch should emit an initial stIoState event");
    let ads_state = configured
        .events
        .iter()
        .filter_map(|value| {
            serde_json::from_value::<Event<crate::protocol::AdsStateEventBody>>(value.clone()).ok()
        })
        .find(|event| event.event == "stAdsState")
        .and_then(|event| event.body)
        .expect("launch should emit an initial stAdsState event");
    assert_eq!(ads_state.schema_version, 1);
    assert_eq!(Some(ads_state.scan), io_state.scan);
    assert!(ads_state.entries.is_empty());
    assert_eq!(
        io_state.inputs[0].source.as_deref(),
        Some("Modbus 127.0.0.1:1502 · input reg 0"),
        "launch stIoState should carry configured input source provenance"
    );
    assert_eq!(
        io_state.outputs[0].source.as_deref(),
        Some("Modbus 127.0.0.1:1502 · output reg 1"),
        "launch stIoState should carry configured output source provenance"
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
