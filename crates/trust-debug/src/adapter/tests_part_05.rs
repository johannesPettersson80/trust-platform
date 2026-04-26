use super::*;

#[test]
fn dispatch_threads_stack_scopes_variables() {
    let mut runtime = Runtime::new();
    let frame_id = runtime.storage_mut().push_frame("MAIN");
    runtime
        .storage_mut()
        .set_local("foo", RuntimeValue::Int(42));
    runtime
        .storage_mut()
        .set_global("g", RuntimeValue::Bool(true));
    runtime.storage_mut().set_retain("r", RuntimeValue::DInt(7));
    let mut fields = IndexMap::new();
    fields.insert(SmolStr::new("field"), RuntimeValue::Bool(true));
    runtime.storage_mut().set_local(
        "s",
        RuntimeValue::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
            SmolStr::new("MY_STRUCT"),
            fields,
        ))),
    );
    runtime.storage_mut().set_local(
        "arr",
        RuntimeValue::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![RuntimeValue::Int(1), RuntimeValue::Int(2)],
                vec![(1, 2)],
            )
            .expect("valid debug array"),
        )),
    );
    let parent_id = runtime.storage_mut().create_instance("ParentFB");
    runtime
        .storage_mut()
        .set_instance_var(parent_id, "pv", RuntimeValue::Bool(false));
    let instance_id = runtime.storage_mut().create_instance("MyFB");
    if let Some(instance) = runtime.storage_mut().get_instance_mut(instance_id) {
        instance.parent = Some(parent_id);
    }
    runtime
        .storage_mut()
        .set_instance_var(instance_id, "iv", RuntimeValue::DInt(3));
    runtime
        .storage_mut()
        .set_local("inst", RuntimeValue::Instance(instance_id));
    let value_ref = runtime.storage().ref_for_local("foo").unwrap();
    runtime
        .storage_mut()
        .set_local("ref", RuntimeValue::Reference(Some(value_ref)));

    let control = DebugControl::new();
    let mut hook = control.clone();
    hook.on_statement(Some(&SourceLocation::new(0, 0, 5)), 0);

    let mut session = DebugSession::with_control(runtime, control);
    session.register_source("main.st", 0, "foo := 1;\n");
    let mut adapter = DebugAdapter::new(session);

    let threads_req = Request::<serde_json::Value> {
        seq: 1,
        message_type: MessageType::Request,
        command: "threads".to_string(),
        arguments: None,
    };
    let threads_outcome = adapter.dispatch_request(threads_req);
    let threads_response: Response<ThreadsResponseBody> =
        serde_json::from_value(threads_outcome.responses[0].clone()).unwrap();
    assert_eq!(threads_response.body.unwrap().threads.len(), 1);

    let stack_req = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "stackTrace".to_string(),
        arguments: Some(
            serde_json::to_value(StackTraceArguments {
                thread_id: 1,
                start_frame: None,
                levels: None,
            })
            .unwrap(),
        ),
    };
    let stack_outcome = adapter.dispatch_request(stack_req);
    let stack_response: Response<StackTraceResponseBody> =
        serde_json::from_value(stack_outcome.responses[0].clone()).unwrap();
    let stack_frames = stack_response.body.unwrap().stack_frames;
    assert_eq!(stack_frames.len(), 1);
    assert_eq!(stack_frames[0].id, frame_id.0);
    assert_eq!(stack_frames[0].line, 1);
    assert_eq!(stack_frames[0].column, 1);

    let scopes_req = Request {
        seq: 3,
        message_type: MessageType::Request,
        command: "scopes".to_string(),
        arguments: Some(
            serde_json::to_value(ScopesArguments {
                frame_id: frame_id.0,
            })
            .unwrap(),
        ),
    };
    let scopes_outcome = adapter.dispatch_request(scopes_req);
    let scopes_response: Response<ScopesResponseBody> =
        serde_json::from_value(scopes_outcome.responses[0].clone()).unwrap();
    let scopes = scopes_response.body.unwrap().scopes;
    let locals_scope = scopes.iter().find(|scope| scope.name == "Locals").unwrap();
    let globals_scope = scopes.iter().find(|scope| scope.name == "Globals").unwrap();
    let instances_scope = scopes
        .iter()
        .find(|scope| scope.name == "Instances")
        .unwrap();

    let locals_req = Request {
        seq: 4,
        message_type: MessageType::Request,
        command: "variables".to_string(),
        arguments: Some(
            serde_json::to_value(VariablesArguments {
                variables_reference: locals_scope.variables_reference,
            })
            .unwrap(),
        ),
    };
    let locals_outcome = adapter.dispatch_request(locals_req);
    let locals_response: Response<VariablesResponseBody> =
        serde_json::from_value(locals_outcome.responses[0].clone()).unwrap();
    let local_vars = locals_response.body.unwrap().variables;
    assert!(local_vars.iter().any(|var| var.name == "foo"));
    let struct_ref = local_vars
        .iter()
        .find(|var| var.name == "s")
        .unwrap()
        .variables_reference;
    let array_ref = local_vars
        .iter()
        .find(|var| var.name == "arr")
        .unwrap()
        .variables_reference;
    let instance_ref = local_vars
        .iter()
        .find(|var| var.name == "inst")
        .unwrap()
        .variables_reference;
    let ref_ref = local_vars
        .iter()
        .find(|var| var.name == "ref")
        .unwrap()
        .variables_reference;
    assert!(struct_ref > 0);
    assert!(array_ref > 0);
    assert!(instance_ref > 0);
    assert!(ref_ref > 0);

    let globals_req = Request {
        seq: 5,
        message_type: MessageType::Request,
        command: "variables".to_string(),
        arguments: Some(
            serde_json::to_value(VariablesArguments {
                variables_reference: globals_scope.variables_reference,
            })
            .unwrap(),
        ),
    };
    let globals_outcome = adapter.dispatch_request(globals_req);
    let globals_response: Response<VariablesResponseBody> =
        serde_json::from_value(globals_outcome.responses[0].clone()).unwrap();
    let global_vars = globals_response.body.unwrap().variables;
    assert!(global_vars.iter().any(|var| var.name == "g"));

    let struct_vars_req = Request {
        seq: 6,
        message_type: MessageType::Request,
        command: "variables".to_string(),
        arguments: Some(
            serde_json::to_value(VariablesArguments {
                variables_reference: struct_ref,
            })
            .unwrap(),
        ),
    };
    let struct_outcome = adapter.dispatch_request(struct_vars_req);
    let struct_response: Response<VariablesResponseBody> =
        serde_json::from_value(struct_outcome.responses[0].clone()).unwrap();
    let struct_vars = struct_response.body.unwrap().variables;
    assert!(struct_vars.iter().any(|var| var.name == "field"));

    let array_vars_req = Request {
        seq: 7,
        message_type: MessageType::Request,
        command: "variables".to_string(),
        arguments: Some(
            serde_json::to_value(VariablesArguments {
                variables_reference: array_ref,
            })
            .unwrap(),
        ),
    };
    let array_outcome = adapter.dispatch_request(array_vars_req);
    let array_response: Response<VariablesResponseBody> =
        serde_json::from_value(array_outcome.responses[0].clone()).unwrap();
    let array_vars = array_response.body.unwrap().variables;
    assert!(array_vars.iter().any(|var| var.name == "[1]"));
    assert!(array_vars.iter().any(|var| var.name == "[2]"));

    let instance_vars_req = Request {
        seq: 8,
        message_type: MessageType::Request,
        command: "variables".to_string(),
        arguments: Some(
            serde_json::to_value(VariablesArguments {
                variables_reference: instance_ref,
            })
            .unwrap(),
        ),
    };
    let instance_outcome = adapter.dispatch_request(instance_vars_req);
    let instance_response: Response<VariablesResponseBody> =
        serde_json::from_value(instance_outcome.responses[0].clone()).unwrap();
    let instance_vars = instance_response.body.unwrap().variables;
    assert!(instance_vars.iter().any(|var| var.name == "iv"));
    let parent_ref = instance_vars
        .iter()
        .find(|var| var.name == "parent")
        .unwrap()
        .variables_reference;
    assert!(parent_ref > 0);

    let parent_vars_req = Request {
        seq: 9,
        message_type: MessageType::Request,
        command: "variables".to_string(),
        arguments: Some(
            serde_json::to_value(VariablesArguments {
                variables_reference: parent_ref,
            })
            .unwrap(),
        ),
    };
    let parent_outcome = adapter.dispatch_request(parent_vars_req);
    let parent_response: Response<VariablesResponseBody> =
        serde_json::from_value(parent_outcome.responses[0].clone()).unwrap();
    let parent_vars = parent_response.body.unwrap().variables;
    assert!(parent_vars.iter().any(|var| var.name == "pv"));

    let ref_vars_req = Request {
        seq: 10,
        message_type: MessageType::Request,
        command: "variables".to_string(),
        arguments: Some(
            serde_json::to_value(VariablesArguments {
                variables_reference: ref_ref,
            })
            .unwrap(),
        ),
    };
    let ref_outcome = adapter.dispatch_request(ref_vars_req);
    let ref_response: Response<VariablesResponseBody> =
        serde_json::from_value(ref_outcome.responses[0].clone()).unwrap();
    let ref_vars = ref_response.body.unwrap().variables;
    assert!(ref_vars.iter().any(|var| var.name == "*"));

    let instances_req = Request {
        seq: 11,
        message_type: MessageType::Request,
        command: "variables".to_string(),
        arguments: Some(
            serde_json::to_value(VariablesArguments {
                variables_reference: instances_scope.variables_reference,
            })
            .unwrap(),
        ),
    };
    let instances_outcome = adapter.dispatch_request(instances_req);
    let instances_response: Response<VariablesResponseBody> =
        serde_json::from_value(instances_outcome.responses[0].clone()).unwrap();
    let instances_vars = instances_response.body.unwrap().variables;
    assert!(instances_vars.iter().any(|var| var.name.contains("MyFB#")));
}

#[test]
fn stack_trace_falls_back_to_main_frame_when_no_storage_frames_exist() {
    let runtime = Runtime::new();
    let control = DebugControl::new();
    let mut hook = control.clone();
    hook.on_statement(Some(&SourceLocation::new(0, 0, 5)), 0);

    let mut session = DebugSession::with_control(runtime, control);
    session.register_source("main.st", 0, "PROGRAM Main\nx := 1;\nEND_PROGRAM\n");
    let mut adapter = DebugAdapter::new(session);

    let stack_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stackTrace".to_string(),
        arguments: Some(
            serde_json::to_value(StackTraceArguments {
                thread_id: 1,
                start_frame: None,
                levels: None,
            })
            .unwrap(),
        ),
    };

    let stack_outcome = adapter.dispatch_request(stack_req);
    let stack_response: Response<StackTraceResponseBody> =
        serde_json::from_value(stack_outcome.responses[0].clone()).unwrap();
    let frames = stack_response.body.unwrap().stack_frames;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].name, "Main");
    assert_eq!(frames[0].line, 1);
}

#[test]
fn stack_trace_falls_back_to_main_frame_when_thread_id_mismatches() {
    let runtime = Runtime::new();
    let control = DebugControl::new();
    let mut hook = control.clone();
    hook.on_statement(Some(&SourceLocation::new(0, 0, 5)), 0);

    let mut session = DebugSession::with_control(runtime, control);
    session.register_source("main.st", 0, "PROGRAM Main\nx := 1;\nEND_PROGRAM\n");
    let mut adapter = DebugAdapter::new(session);

    let stack_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stackTrace".to_string(),
        arguments: Some(
            serde_json::to_value(StackTraceArguments {
                thread_id: 99,
                start_frame: None,
                levels: None,
            })
            .unwrap(),
        ),
    };

    let stack_outcome = adapter.dispatch_request(stack_req);
    let stack_response: Response<StackTraceResponseBody> =
        serde_json::from_value(stack_outcome.responses[0].clone()).unwrap();
    let frames = stack_response.body.unwrap().stack_frames;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].name, "Main");
}

#[test]
fn stack_trace_returns_default_main_frame_without_location() {
    let runtime = Runtime::new();
    let control = DebugControl::new();

    let mut session = DebugSession::with_control(runtime, control);
    session.register_source("main.st", 0, "PROGRAM Main\nx := 1;\nEND_PROGRAM\n");
    let mut adapter = DebugAdapter::new(session);

    let stack_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stackTrace".to_string(),
        arguments: Some(
            serde_json::to_value(StackTraceArguments {
                thread_id: 1,
                start_frame: None,
                levels: None,
            })
            .unwrap(),
        ),
    };

    let stack_outcome = adapter.dispatch_request(stack_req);
    let stack_response: Response<StackTraceResponseBody> =
        serde_json::from_value(stack_outcome.responses[0].clone()).unwrap();
    let frames = stack_response.body.unwrap().stack_frames;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].name, "Main");
    assert_eq!(frames[0].line, 1);
    assert_eq!(frames[0].column, 1);
}

#[test]
fn scopes_for_synthetic_main_frame_are_graceful() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("g", RuntimeValue::Bool(true));
    let control = DebugControl::new();

    let mut session = DebugSession::with_control(runtime, control);
    session.register_source("main.st", 0, "PROGRAM Main\nx := 1;\nEND_PROGRAM\n");
    let mut adapter = DebugAdapter::new(session);

    let stack_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stackTrace".to_string(),
        arguments: Some(
            serde_json::to_value(StackTraceArguments {
                thread_id: 1,
                start_frame: None,
                levels: None,
            })
            .unwrap(),
        ),
    };
    let stack_outcome = adapter.dispatch_request(stack_req);
    let stack_response: Response<StackTraceResponseBody> =
        serde_json::from_value(stack_outcome.responses[0].clone()).unwrap();
    let frames = stack_response.body.unwrap().stack_frames;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].id, 0);

    let scopes_req = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "scopes".to_string(),
        arguments: Some(serde_json::to_value(ScopesArguments { frame_id: 0 }).unwrap()),
    };
    let scopes_outcome = adapter.dispatch_request(scopes_req);
    let scopes_response: Response<ScopesResponseBody> =
        serde_json::from_value(scopes_outcome.responses[0].clone()).unwrap();
    let scopes = scopes_response.body.unwrap().scopes;
    assert!(!scopes.iter().any(|scope| scope.name == "Locals"));
    assert!(scopes.iter().any(|scope| scope.name == "Globals"));
}

#[test]
fn io_state_then_stack_trace_do_not_block_when_runtime_mutex_is_held() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("g", RuntimeValue::Bool(true));
    let control = DebugControl::new();
    let mut hook = control.clone();
    hook.on_statement(Some(&SourceLocation::new(0, 0, 5)), 0);
    control.refresh_snapshot_from_storage(runtime.storage(), runtime.current_time());

    let mut session = DebugSession::with_control(runtime, control);
    session.register_source("main.st", 0, "PROGRAM Main\nx := 1;\nEND_PROGRAM\n");
    let mut adapter = DebugAdapter::new(session);
    let runtime_handle = adapter.session().runtime_handle();

    let runtime_guard = runtime_handle.lock().expect("runtime lock");
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let io_req = Request::<serde_json::Value> {
            seq: 1,
            message_type: MessageType::Request,
            command: "stIoState".to_string(),
            arguments: None,
        };
        let io_outcome = adapter.dispatch_request(io_req);

        let stack_req = Request {
            seq: 2,
            message_type: MessageType::Request,
            command: "stackTrace".to_string(),
            arguments: Some(
                serde_json::to_value(StackTraceArguments {
                    thread_id: 1,
                    start_frame: None,
                    levels: None,
                })
                .unwrap(),
            ),
        };
        let stack_outcome = adapter.dispatch_request(stack_req);
        let _ = tx.send((io_outcome.responses, stack_outcome.responses));
    });

    let result = rx.recv_timeout(std::time::Duration::from_millis(300));
    drop(runtime_guard);

    let (io_responses, stack_responses) =
        result.expect("stIoState + stackTrace should not block while runtime mutex is held");
    handle.join().expect("worker thread joins");

    let io_response: Response<IoStateEventBody> =
        serde_json::from_value(io_responses[0].clone()).unwrap();
    assert!(io_response.success);

    let stack_response: Response<StackTraceResponseBody> =
        serde_json::from_value(stack_responses[0].clone()).unwrap();
    let frames = stack_response.body.unwrap().stack_frames;
    assert!(!frames.is_empty());
    assert_eq!(frames[0].name, "Main");
}
