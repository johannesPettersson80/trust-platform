use super::*;

#[test]
fn stdio_roundtrip() {
    let payload = r#"{\"seq\":1,\"type\":\"request\",\"command\":\"initialize\"}"#;
    let mut buffer = Vec::new();
    write_message(&mut buffer, payload).unwrap();

    let mut reader = BufReader::new(&buffer[..]);
    let read = read_message(&mut reader).unwrap().unwrap();
    assert_eq!(read, payload);
}

#[test]
fn dispatch_set_breakpoints_returns_adjusted_positions() {
    let mut runtime = Runtime::new();
    let source = "x := 1;\n  y := 2;\n";
    let x_start = source.find("x := 1;").unwrap();
    let x_end = x_start + "x := 1;".len();
    let y_start = source.find("y := 2;").unwrap();
    let y_end = y_start + "y := 2;".len();
    runtime.register_statement_locations(
        0,
        vec![
            SourceLocation::new(0, x_start as u32, x_end as u32),
            SourceLocation::new(0, y_start as u32, y_end as u32),
        ],
    );

    let mut session = DebugSession::new(runtime);
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);

    let args = SetBreakpointsArguments {
        source: Source {
            name: Some("main".into()),
            path: Some("main.st".into()),
            source_reference: None,
        },
        breakpoints: Some(vec![SourceBreakpoint {
            line: 2,
            column: Some(1),
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

    let outcome = adapter.dispatch_request(request);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<SetBreakpointsResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    let breakpoint = &response.body.unwrap().breakpoints[0];
    assert!(breakpoint.verified);
    assert_eq!(breakpoint.line, Some(2));
    assert_eq!(breakpoint.column, Some(3));
}

#[test]
fn dispatch_set_breakpoints_in_if_block_targets_inner_stmt() {
    let source = r#"PROGRAM Main
VAR
    x : BOOL := TRUE;
    y : INT := 0;
END_VAR
IF x THEN
    y := y + 1;
END_IF;
END_PROGRAM
"#;
    let harness = TestHarness::from_source(source).unwrap();
    let mut session = DebugSession::new(harness.into_runtime());
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);

    let line_index = source
        .lines()
        .position(|line| line.contains("y := y + 1;"))
        .unwrap();
    let line = line_index as u32 + 1;
    let column = source
        .lines()
        .nth(line_index)
        .unwrap()
        .find("y := y + 1;")
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
            column: Some(1),
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
    let outcome = adapter.dispatch_request(request);
    let response: Response<SetBreakpointsResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    let breakpoint = &response.body.unwrap().breakpoints[0];
    assert!(breakpoint.verified);
    assert_eq!(breakpoint.line, Some(line));
    assert_eq!(breakpoint.column, Some(column));
}

#[test]
fn dispatch_breakpoint_locations_returns_statement_starts() {
    let mut runtime = Runtime::new();
    let source = "x := 1;\n  y := 2;\n";
    let x_start = source.find("x := 1;").unwrap();
    let x_end = x_start + "x := 1;".len();
    let y_start = source.find("y := 2;").unwrap();
    let y_end = y_start + "y := 2;".len();
    runtime.register_statement_locations(
        0,
        vec![
            SourceLocation::new(0, x_start as u32, x_end as u32),
            SourceLocation::new(0, y_start as u32, y_end as u32),
        ],
    );

    let mut session = DebugSession::new(runtime);
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);

    let args = BreakpointLocationsArguments {
        source: Source {
            name: Some("main".into()),
            path: Some("main.st".into()),
            source_reference: None,
        },
        line: 2,
        column: Some(1),
        end_line: None,
        end_column: None,
    };

    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "breakpointLocations".to_string(),
        arguments: Some(serde_json::to_value(args).unwrap()),
    };

    let outcome = adapter.dispatch_request(request);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<BreakpointLocationsResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    let breakpoints = response.body.unwrap().breakpoints;
    assert_eq!(breakpoints.len(), 1);
    assert_eq!(breakpoints[0].line, 2);
    assert_eq!(breakpoints[0].column, Some(3));
}

#[test]
fn dispatch_io_state_emits_event() {
    let mut runtime = Runtime::new();
    let input_addr = IoAddress::parse("%IX0.0").unwrap();
    let output_addr = IoAddress::parse("%QX0.1").unwrap();
    let speed_addr = IoAddress::parse("%MD0").unwrap();
    let mut label_addr = IoAddress::parse("%IB8").unwrap();
    label_addr.size = trust_runtime::io::IoSize::Bytes(12);
    runtime.io_mut().bind("IN0", input_addr.clone());
    runtime.io_mut().bind("OUT0", output_addr.clone());
    runtime
        .io_mut()
        .bind_typed("Speed", speed_addr.clone(), trust_hir::TypeId::REAL);
    runtime
        .io_mut()
        .bind_typed("Label", label_addr.clone(), trust_hir::TypeId::STRING);
    runtime
        .io_mut()
        .write(&input_addr, RuntimeValue::Bool(true))
        .unwrap();
    runtime
        .io_mut()
        .write(&output_addr, RuntimeValue::Bool(false))
        .unwrap();
    runtime
        .io_mut()
        .write(&speed_addr, RuntimeValue::DWord(0x3FC0_0000))
        .unwrap();
    runtime
        .io_mut()
        .write(&label_addr, RuntimeValue::String("Ready".into()))
        .unwrap();

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let request = Request::<serde_json::Value> {
        seq: 1,
        message_type: MessageType::Request,
        command: "stIoState".to_string(),
        arguments: None,
    };

    let outcome = adapter.dispatch_request(request);
    assert_eq!(outcome.events.len(), 1);
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert_eq!(event.event, "stIoState");
    let body = event.body.unwrap();
    assert!(body
        .inputs
        .iter()
        .any(|entry| entry.name.as_deref() == Some("IN0")));
    assert!(body
        .outputs
        .iter()
        .any(|entry| entry.name.as_deref() == Some("OUT0")));
    assert!(body.memory.iter().any(|entry| {
        entry.name.as_deref() == Some("Speed")
            && entry.value_type.as_deref() == Some("REAL")
            && entry.value == "1.5"
    }));
    assert!(body.inputs.iter().any(|entry| {
        entry.name.as_deref() == Some("Label")
            && entry.value_type.as_deref() == Some("STRING")
            && entry.value == "Ready"
    }));
}

#[test]
fn dispatch_io_write_updates_input() {
    let mut runtime = Runtime::new();
    let input_addr = IoAddress::parse("%IX0.2").unwrap();
    runtime.io_mut().bind("IN2", input_addr.clone());

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let args = IoWriteArguments {
        address: "%IX0.2".to_string(),
        value: "TRUE".to_string(),
    };
    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stIoWrite".to_string(),
        arguments: Some(serde_json::to_value(args).unwrap()),
    };

    let outcome = adapter.dispatch_request(request);
    assert_eq!(outcome.responses.len(), 1);
    assert_eq!(
        outcome.events.len(),
        0,
        "stIoWrite must wait for an explicit/newer I/O snapshot instead of emitting a partial stale row"
    );

    let value = adapter
        .session()
        .runtime_handle()
        .lock()
        .unwrap()
        .io()
        .read(&input_addr)
        .unwrap();
    assert_eq!(value, RuntimeValue::Bool(true));

    let state_request = Request::<serde_json::Value> {
        seq: 2,
        message_type: MessageType::Request,
        command: "stIoState".to_string(),
        arguments: None,
    };
    let outcome = adapter.dispatch_request(state_request);
    assert_eq!(outcome.events.len(), 1);
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert_eq!(event.event, "stIoState");
    assert!(event
        .body
        .unwrap()
        .inputs
        .iter()
        .any(|entry| entry.address == "%IX0.2" && entry.value == "TRUE"));
}

#[test]
fn dispatch_io_write_accepts_configured_real_and_time_values() {
    let mut runtime = Runtime::new();
    let real_addr = IoAddress::parse("%ID0").unwrap();
    let time_addr = IoAddress::parse("%ID4").unwrap();
    let mut string_addr = IoAddress::parse("%IB8").unwrap();
    string_addr.size = trust_runtime::io::IoSize::Bytes(12);
    runtime
        .io_mut()
        .bind_typed("Speed", real_addr.clone(), trust_hir::TypeId::REAL);
    runtime
        .io_mut()
        .bind_typed("Delay", time_addr.clone(), trust_hir::TypeId::TIME);
    runtime
        .io_mut()
        .bind_typed("Label", string_addr.clone(), trust_hir::TypeId::STRING);

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    for (seq, address, value) in [
        (1, "%ID0", "1.5"),
        (2, "%ID4", "T#250ms"),
        (3, "%IB8", "Running"),
    ] {
        let args = IoWriteArguments {
            address: address.to_string(),
            value: value.to_string(),
        };
        let request = Request {
            seq,
            message_type: MessageType::Request,
            command: "stIoWrite".to_string(),
            arguments: Some(serde_json::to_value(args).unwrap()),
        };
        let outcome = adapter.dispatch_request(request);
        assert_eq!(outcome.responses.len(), 1);
        let response: Response<serde_json::Value> =
            serde_json::from_value(outcome.responses[0].clone()).unwrap();
        assert!(response.success, "typed I/O write failed: {response:?}");
    }

    let runtime_handle = adapter.session().runtime_handle();
    let runtime = runtime_handle.lock().unwrap();
    assert_eq!(
        runtime.io().read(&real_addr).unwrap(),
        RuntimeValue::DWord(1.5f32.to_bits())
    );
    assert_eq!(
        runtime.io().read(&time_addr).unwrap(),
        RuntimeValue::DWord(250)
    );
    assert_eq!(
        runtime.io().read(&string_addr).unwrap(),
        RuntimeValue::String("Running".into())
    );
}
