use super::*;

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
