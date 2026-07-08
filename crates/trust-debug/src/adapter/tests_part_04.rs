use super::*;

#[test]
fn reload_while_runner_active_does_not_emit_pre_scan_io_state() {
    let root = std::env::temp_dir().join(format!(
        "trust-debug-reload-running-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("main.st");
    let source_v1 = r#"
CONFIGURATION Conf
TASK Cycle (INTERVAL := T#20ms, PRIORITY := 1);
PROGRAM P1 WITH Cycle : Main;
END_CONFIGURATION

PROGRAM Main
VAR
    Output : BOOL;
END_VAR
Output := FALSE;
END_PROGRAM
"#;
    let source_v2 = source_v1.replace("Output := FALSE;", "Output := TRUE;");
    std::fs::write(&path, source_v1).unwrap();

    let harness = TestHarness::from_source(source_v1).unwrap();
    let mut session = DebugSession::new(harness.into_runtime());
    session.register_source(path.to_string_lossy(), 0, source_v1);
    session.set_program_path(path.to_string_lossy());
    let mut adapter = DebugAdapter::new(session);

    adapter.start_runner();
    std::fs::write(&path, source_v2).unwrap();
    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stReload".to_string(),
        arguments: Some(serde_json::json!({ "program": path })),
    };

    let outcome = adapter.dispatch_request(request);
    adapter.stop_runner();

    assert_eq!(outcome.responses.len(), 1);
    let response: Response<Value> = serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "reload should succeed: {:?}",
        response.message
    );
    assert!(
        outcome
            .events
            .iter()
            .all(|event| { event.get("event").and_then(Value::as_str) != Some("stIoState") }),
        "running reload must wait for the next scan instead of emitting a pre-scan I/O state: {:?}",
        outcome.events
    );
}

#[test]
fn reload_while_runner_active_reports_coherent_conveyor_io_state() {
    let root = std::env::temp_dir().join(format!(
        "trust-debug-reload-conveyor-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("main.st");
    let source_v1 = r#"
CONFIGURATION Config
TASK MainTask (INTERVAL := T#20ms, PRIORITY := 1);
PROGRAM P1 WITH MainTask : Main;
END_CONFIGURATION

PROGRAM Main
VAR
    e_stop AT %IX0.0 : BOOL;
    running AT %QX2.0 : BOOL;
    conveyor_speed AT %QW0 : INT;
    part_count AT %MW0 : INT;
END_VAR
running := NOT e_stop;
IF running THEN
    IF conveyor_speed < 100 THEN
        conveyor_speed := conveyor_speed + 5;
    ELSE
        part_count := part_count + 1;
        conveyor_speed := 0;
    END_IF;
ELSE
    conveyor_speed := 0;
END_IF;
END_PROGRAM
"#;
    let source_v2 = source_v1.replace(
        "conveyor_speed := conveyor_speed + 5;",
        "conveyor_speed := 42;",
    );
    std::fs::write(&path, source_v1).unwrap();

    let harness = TestHarness::from_source(source_v1).unwrap();
    let mut session = DebugSession::new(harness.into_runtime());
    session.register_source(path.to_string_lossy(), 0, source_v1);
    session.set_program_path(path.to_string_lossy());
    let mut adapter = DebugAdapter::new(session);

    adapter.start_runner();
    std::thread::sleep(std::time::Duration::from_millis(120));
    std::fs::write(&path, source_v2).unwrap();
    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stReload".to_string(),
        arguments: Some(serde_json::json!({ "program": path })),
    };
    let outcome = adapter.dispatch_request(request);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<Value> = serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "reload should succeed: {:?}",
        response.message
    );

    std::thread::sleep(std::time::Duration::from_millis(500));
    adapter.stop_runner();
    let state = adapter
        .capture_io_state_from_runtime()
        .expect("runtime state after runner stop");
    let all = state
        .inputs
        .iter()
        .chain(state.outputs.iter())
        .chain(state.memory.iter())
        .collect::<Vec<_>>();
    let value = |needle: &str| {
        all.iter()
            .find(|entry| {
                entry
                    .name
                    .as_deref()
                    .is_some_and(|name| name.ends_with(needle))
                    || entry.address == needle
            })
            .map(|entry| entry.value.as_str())
    };
    let observed = serde_json::json!({
        "scan": state.scan,
        "e_stop": value("e_stop").or_else(|| value("%IX0.0")),
        "running": value("running").or_else(|| value("%QX2.0")),
        "conveyor_speed": value("conveyor_speed").or_else(|| value("%QW0")),
    });
    if value("e_stop").or_else(|| value("%IX0.0")) == Some("FALSE")
        && value("running").or_else(|| value("%QX2.0")) == Some("TRUE")
        && value("conveyor_speed").or_else(|| value("%QW0")) == Some("42")
    {
        return;
    }
    panic!(
        "expected coherent post-reload conveyor I/O state, last observed: {}",
        observed
    );
}

#[test]
fn dap_breakpoint_stops_and_resumes_with_task_order() {
    let source = r#"
CONFIGURATION Conf
VAR_GLOBAL
trigger1 : BOOL := FALSE;
trigger2 : BOOL := FALSE;
trace : INT := 0;
END_VAR
TASK Fast (SINGLE := trigger1, PRIORITY := 1);
TASK Slow (SINGLE := trigger2, PRIORITY := 2);
PROGRAM P1 WITH Fast : Prog1;
PROGRAM P2 WITH Slow : Prog2;
END_CONFIGURATION

PROGRAM Prog1
trace := trace * INT#10 + INT#1;
END_PROGRAM

PROGRAM Prog2
trace := trace * INT#10 + INT#2;
END_PROGRAM
"#;

    let harness = TestHarness::from_source(source).unwrap();
    let mut session = DebugSession::new(harness.into_runtime());
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);

    let line = source
        .lines()
        .position(|line| line.contains("trace := trace * INT#10 + INT#1;"))
        .unwrap() as u32
        + 1;
    let args = SetBreakpointsArguments {
        source: Source {
            name: Some("main".into()),
            path: Some("main.st".into()),
            source_reference: None,
        },
        breakpoints: Some(vec![SourceBreakpoint {
            line,
            column: None,
            condition: None,
            hit_condition: None,
            log_message: None,
        }]),
        lines: None,
        source_modified: None,
    };
    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setBreakpoints".to_string(),
        arguments: Some(serde_json::to_value(args).unwrap()),
    };
    adapter.dispatch_request(request);

    let control = adapter.session().debug_control();
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    control.set_stop_sender(stop_tx);

    let session = adapter.into_session();
    let runtime = session.runtime_handle();
    {
        let mut guard = runtime.lock().unwrap();
        guard
            .storage_mut()
            .set_global("trigger1", RuntimeValue::Bool(true));
        guard
            .storage_mut()
            .set_global("trigger2", RuntimeValue::Bool(true));
    }

    let runtime_thread = Arc::clone(&runtime);
    let handle = std::thread::spawn(move || {
        let mut guard = runtime_thread.lock().unwrap();
        guard.execute_cycle().unwrap();
    });

    let stop = stop_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);
    control.continue_run();

    handle.join().unwrap();
    let guard = runtime.lock().unwrap();
    assert_eq!(
        guard.storage().get_global("trace"),
        Some(&RuntimeValue::Int(12))
    );
}
