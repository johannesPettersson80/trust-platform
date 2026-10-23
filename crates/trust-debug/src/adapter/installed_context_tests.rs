//! Actual admitted execution, detached DAP inspection and queued write completion.
use super::*;
use crate::protocol::{SetVariableArguments, SetVariableResponseBody};
use trust_runtime::{
    debug::RuntimeEvent,
    harness::{CompileSession, SourceFile},
    program_model::Expr,
};

#[test]
fn schema_reload_rebuilds_watch_plans_only_after_candidate_commit() {
    let root = std::env::temp_dir().join(format!(
        "trust-watch-schema-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("main.st");
    let source = "TYPE Packet : INT; END_TYPE PROGRAM Main VAR value : Packet; END_VAR value := value; END_PROGRAM";
    std::fs::write(&path, source).unwrap();
    let runtime =
        CompileSession::from_sources(vec![SourceFile::with_path(path.to_string_lossy(), source)])
            .build_runtime()
            .unwrap();
    let mut session = DebugSession::new(runtime);
    session.register_source(path.to_string_lossy(), 0, source);
    session.set_program_path(path.to_string_lossy());
    let mut adapter = DebugAdapter::new(session);
    let evaluate = |adapter: &mut DebugAdapter| {
        let outcome = adapter.dispatch_request(Request {
            seq: 1,
            message_type: MessageType::Request,
            command: "evaluate".into(),
            arguments: Some(serde_json::json!({"expression":"SIZEOF(Packet)","context":"watch"})),
        });
        let response: Response<EvaluateResponseBody> =
            serde_json::from_value(outcome.responses[0].clone()).unwrap();
        assert!(response.success, "{response:?}");
        response.body.unwrap().result
    };
    let reload = |adapter: &mut DebugAdapter| {
        let outcome = adapter.dispatch_request(Request {
            seq: 2,
            message_type: MessageType::Request,
            command: "stReload".into(),
            arguments: Some(serde_json::json!({"program":path})),
        });
        let response: Response<Value> =
            serde_json::from_value(outcome.responses[0].clone()).unwrap();
        response.success
    };
    assert_eq!(evaluate(&mut adapter), "2");
    assert_eq!(adapter.watch_cache.len(), 1);
    let old_handle = adapter.alloc_variable_handle(VariableHandle::Globals);
    let control = adapter.session().debug_control();
    adapter
        .session()
        .runtime_handle()
        .lock()
        .unwrap()
        .execute_cycle()
        .unwrap();
    assert!(control.take_watch_changed());
    assert!(control.snapshot().is_some());

    std::fs::write(&path, "PROGRAM Broken").unwrap();
    assert!(!reload(&mut adapter));
    assert_eq!(
        adapter.watch_cache.len(),
        1,
        "failed candidate keeps the old compiled watch"
    );
    assert!(adapter.variable_handles.contains_key(&old_handle));
    assert!(control.snapshot().is_some());
    assert_eq!(evaluate(&mut adapter), "2");

    std::fs::write(&path, source.replace("Packet : INT", "Packet : LINT")).unwrap();
    assert!(reload(&mut adapter));
    assert!(adapter.watch_cache.is_empty());
    assert!(adapter.variable_handles.is_empty());
    assert!(control.snapshot().is_none());
    adapter
        .session()
        .runtime_handle()
        .lock()
        .unwrap()
        .execute_cycle()
        .unwrap();
    assert!(
        !control.take_watch_changed(),
        "retired schema must not retain an active watch plan"
    );
    assert_eq!(evaluate(&mut adapter), "8");
    adapter
        .session()
        .runtime_handle()
        .lock()
        .unwrap()
        .execute_cycle()
        .unwrap();
    assert!(
        control.take_watch_changed(),
        "normal evaluation reinstalls the current watch plan"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn dap_anonymous_type_breakpoint_and_logpoint_use_the_parser_registry() {
    let source = "PROGRAM Main\nVAR x : INT := 0; END_VAR\nx := x + 1;\nEND_PROGRAM\n";
    let runtime = CompileSession::from_sources(vec![SourceFile::with_path("main.st", source)])
        .build_runtime()
        .unwrap();
    let mut session = DebugSession::new(runtime);
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);
    let control = adapter.session().debug_control();
    let line = source
        .lines()
        .position(|line| line.contains("x := x + 1;"))
        .unwrap() as u32
        + 1;
    let mut request_sequence = 0;
    let mut set_breakpoint =
        |adapter: &mut DebugAdapter,
         condition: Option<&str>,
         log_message: Option<&str>,
         breakpoints: Option<Vec<SourceBreakpoint>>| {
            request_sequence += 1;
            let breakpoints = breakpoints.unwrap_or_else(|| {
                vec![SourceBreakpoint {
                    line,
                    column: Some(1),
                    condition: condition.map(str::to_string),
                    hit_condition: None,
                    log_message: log_message.map(str::to_string),
                }]
            });
            let outcome = adapter.dispatch_request(Request {
                seq: request_sequence,
                message_type: MessageType::Request,
                command: "setBreakpoints".into(),
                arguments: Some(
                    serde_json::to_value(SetBreakpointsArguments {
                        source: Source {
                            name: Some("main.st".into()),
                            path: Some("main.st".into()),
                            source_reference: None,
                        },
                        breakpoints: Some(breakpoints),
                        lines: None,
                        source_modified: None,
                    })
                    .unwrap(),
                ),
            });
            serde_json::from_value::<Response<SetBreakpointsResponseBody>>(
                outcome.responses[0].clone(),
            )
            .unwrap()
        };

    let response = set_breakpoint(&mut adapter, Some("SIZEOF(STRING[17]) = 17"), None, None);
    assert!(response.success);
    assert!(response.body.unwrap().breakpoints[0].verified);
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    control.set_stop_sender(stop_tx);
    let runtime_handle = adapter.session().runtime_handle();
    let executing = Arc::clone(&runtime_handle);
    let thread = std::thread::spawn(move || executing.lock().unwrap().execute_cycle());
    let stop = stop_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("DAP-installed anonymous-type condition must stop the running engine");
    control.continue_run();
    thread.join().unwrap().unwrap();
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);

    let cleared = set_breakpoint(&mut adapter, None, None, Some(Vec::new()));
    assert!(cleared.success);
    assert!(cleared.body.unwrap().breakpoints.is_empty());
    let response = set_breakpoint(&mut adapter, None, Some("size={SIZEOF(STRING[17])}"), None);
    assert!(response.success);
    assert!(response.body.unwrap().breakpoints[0].verified);
    adapter
        .session()
        .runtime_handle()
        .lock()
        .unwrap()
        .execute_cycle()
        .unwrap();
    assert!(control
        .drain_logs()
        .iter()
        .any(|log| log.message == "size=DInt(17)"));
    assert!(adapter
        .session()
        .runtime_handle()
        .lock()
        .unwrap()
        .registry()
        .lookup("STRING[17]")
        .is_none());
}

#[test]
fn paused_installed_calls_preserve_ancestor_locals_scratch_watches_and_stale_write_reporting() {
    let source = r#"
FUNCTION Inner : DINT
VAR_INPUT input_value : DINT; END_VAR
VAR temporary : DINT; END_VAR
temporary := input_value + 1;
Inner := temporary;
END_FUNCTION
FUNCTION Outer : DINT
VAR_INPUT input_value : DINT; END_VAR
VAR carry : DINT; END_VAR
carry := input_value + 2;
Outer := Inner(carry);
END_FUNCTION
PROGRAM Main
VAR result : DINT; END_VAR
result := Outer(3);
END_PROGRAM
"#;
    let runtime = CompileSession::from_sources(vec![SourceFile::with_path("main.st", source)])
        .build_runtime()
        .unwrap();
    let mut session = DebugSession::new(runtime);
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);
    let control = adapter.session().debug_control();
    control.register_watch_expression(Expr::Name("temporary".into()));
    let line = source
        .lines()
        .position(|line| line.contains("Inner := temporary;"))
        .unwrap() as u32
        + 1;
    let outcome = adapter.dispatch_request(Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setBreakpoints".into(),
        arguments: Some(
            serde_json::to_value(SetBreakpointsArguments {
                source: Source {
                    name: Some("main.st".into()),
                    path: Some("main.st".into()),
                    source_reference: None,
                },
                breakpoints: Some(vec![SourceBreakpoint {
                    line,
                    column: Some(1),
                    condition: Some("temporary = 6".into()),
                    hit_condition: None,
                    log_message: None,
                }]),
                lines: None,
                source_modified: None,
            })
            .unwrap(),
        ),
    });
    let response: Response<SetBreakpointsResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.body.unwrap().breakpoints[0].verified);
    let (sender, receiver) = std::sync::mpsc::channel();
    control.set_stop_sender(sender);
    let runtime_handle = adapter.session().runtime_handle();
    let executing = Arc::clone(&runtime_handle);
    let thread = std::thread::spawn(move || executing.lock().unwrap().execute_cycle());

    // Release the owner even if a DAP/native assertion fails while it is paused.
    let inspected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let stop = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("actual inner breakpoint");
        assert_eq!(stop.reason, DebugStopReason::Breakpoint);
        let snapshot = control
            .snapshot()
            .expect("current and suspended invocation projection");
        let inner = snapshot
            .storage
            .frames()
            .iter()
            .find(|frame| frame.owner.eq_ignore_ascii_case("Inner"))
            .unwrap();
        let outer = snapshot
            .storage
            .frames()
            .iter()
            .find(|frame| frame.owner.eq_ignore_ascii_case("Outer"))
            .unwrap();
        assert_eq!(
            inner.variables.get("temporary"),
            Some(&RuntimeValue::DInt(6))
        );
        assert_eq!(outer.variables.get("carry"), Some(&RuntimeValue::DInt(5)));
        assert_ne!(inner.id, outer.id);
        assert!(
            control.take_watch_changed(),
            "actual contextual watch evaluated before pause"
        );

        let evaluation = adapter.dispatch_request(Request {
            seq: 2,
            message_type: MessageType::Request,
            command: "evaluate".into(),
            arguments: Some(
                serde_json::to_value(EvaluateArguments {
                    expression: "carry".into(),
                    frame_id: Some(outer.id.0),
                    context: Some("watch".into()),
                })
                .unwrap(),
            ),
        });
        let evaluation: Response<EvaluateResponseBody> =
            serde_json::from_value(evaluation.responses[0].clone()).unwrap();
        assert!(evaluation.success);
        assert_eq!(evaluation.body.unwrap().result, "5");
        let scratch = adapter.dispatch_request(Request {
            seq: 3,
            message_type: MessageType::Request,
            command: "evaluate".into(),
            arguments: Some(
                serde_json::to_value(EvaluateArguments {
                    expression: "SIZEOF(STRING[17])".into(),
                    frame_id: Some(inner.id.0),
                    context: Some("watch".into()),
                })
                .unwrap(),
            ),
        });
        let scratch: Response<EvaluateResponseBody> =
            serde_json::from_value(scratch.responses[0].clone()).unwrap();
        assert!(scratch.success);
        assert_eq!(scratch.body.unwrap().result, "17");
        assert!(adapter
            .session()
            .metadata()
            .registry()
            .lookup("STRING[17]")
            .is_none());

        let handle = adapter.alloc_variable_handle(VariableHandle::Locals(inner.id));
        let write = adapter.dispatch_request(Request {
            seq: 4,
            message_type: MessageType::Request,
            command: "setVariable".into(),
            arguments: Some(
                serde_json::to_value(SetVariableArguments {
                    variables_reference: handle,
                    name: "temporary".into(),
                    value: "99".into(),
                })
                .unwrap(),
            ),
        });
        let write: Response<SetVariableResponseBody> =
            serde_json::from_value(write.responses[0].clone()).unwrap();
        assert!(
            write.success,
            "queue acceptance and detached presentation update: {write:?}"
        );
        let projected = control.snapshot().unwrap();
        assert_eq!(
            projected
                .storage
                .frames()
                .iter()
                .find(|frame| frame.id == inner.id)
                .unwrap()
                .variables
                .get("temporary"),
            Some(&RuntimeValue::DInt(99))
        );
        inner.id
    }));
    control.clear_breakpoints();
    control.continue_run();
    thread.join().unwrap().unwrap();
    let retired = match inspected {
        Ok(frame) => frame,
        Err(error) => std::panic::resume_unwind(error),
    };
    let mut runtime = runtime_handle.lock().unwrap();
    let Some(RuntimeValue::Instance(root)) = runtime.storage().get_global("Main") else {
        panic!("program root")
    };
    let root = *root;
    assert_eq!(
        runtime.storage().get_instance_var(root, "result"),
        Some(&RuntimeValue::DInt(6)),
        "detached DAP edits never copy back into a live invocation"
    );
    runtime.advance_time(Duration::from_millis(10));
    runtime.execute_cycle().unwrap();
    assert!(
        !runtime.faulted(),
        "rejected management request is not a PLC fault"
    );
    assert_eq!(
        runtime.storage().get_instance_var(root, "result"),
        Some(&RuntimeValue::DInt(6))
    );
    assert!(runtime.registry().lookup("STRING[17]").is_none());
    assert!(control.drain_runtime_events().iter().any(|event| matches!(event,
        RuntimeEvent::WriteRejected { target, .. } if target.as_str() == format!("{}.temporary", retired.0))));
}

#[test]
fn checked_io_rejection_does_not_install_a_force_or_queue() {
    let mut adapter = DebugAdapter::new(DebugSession::new(
        trust_runtime::RuntimeBuilder::new().build().unwrap(),
    ));
    for command in ["stIoWrite", "stIoForce"] {
        let outcome = adapter.dispatch_request(Request {
            seq: 1,
            message_type: MessageType::Request,
            command: command.into(),
            arguments: Some(
                serde_json::to_value(IoWriteArguments {
                    address: "%IX99.0".into(),
                    value: "TRUE".into(),
                })
                .unwrap(),
            ),
        });
        let response: Response<serde_json::Value> =
            serde_json::from_value(outcome.responses[0].clone()).unwrap();
        assert!(!response.success);
        assert!(adapter
            .session()
            .debug_control()
            .drain_io_writes()
            .is_empty());
        assert!(adapter.forced_io_addresses.lock().unwrap().is_empty());
        let runtime = adapter.session().runtime_handle();
        let mut runtime = runtime.lock().unwrap();
        assert!(runtime.io().inputs().is_empty());
        runtime.execute_cycle().unwrap();
        assert!(!runtime.faulted());
        assert!(
            !adapter
                .session()
                .debug_control()
                .drain_runtime_events()
                .iter()
                .any(|event| matches!(event, RuntimeEvent::WriteRejected { .. })),
            "no rejected force survives to be reapplied during a scan"
        );
    }
}

#[test]
fn reference_variable_inspection_materializes_characters_from_a_detached_snapshot() {
    let mut storage = trust_runtime::memory::VariableStorage::new();
    storage.push_frame("inspection");
    storage.set_local("narrow", RuntimeValue::String("AB".into()));
    storage.set_local("wide", RuntimeValue::WString("αβ".into()));
    let control = DebugControl::new();
    control.refresh_snapshot_from_storage(&storage, Duration::ZERO);
    let mut adapter = DebugAdapter::new(DebugSession::with_control(
        trust_runtime::RuntimeBuilder::new().build().unwrap(),
        control,
    ));
    for (name, character, type_name) in [("narrow", "B", "CHAR"), ("wide", "β", "WCHAR")] {
        let mut reference = storage.ref_for_local(name).unwrap();
        reference
            .path
            .push(trust_runtime::value::RefSegment::Index(vec![2]));
        let handle = adapter.alloc_variable_handle(VariableHandle::Reference(reference));
        let outcome = adapter.dispatch_request(Request {
            seq: 1,
            message_type: MessageType::Request,
            command: "variables".into(),
            arguments: Some(
                serde_json::to_value(VariablesArguments {
                    variables_reference: handle,
                })
                .unwrap(),
            ),
        });
        let response: Response<VariablesResponseBody> =
            serde_json::from_value(outcome.responses[0].clone()).unwrap();
        let variables = response.body.unwrap().variables;
        assert_eq!(variables.len(), 1);
        assert_eq!(variables[0].value, character);
        assert_eq!(variables[0].r#type.as_deref(), Some(type_name));
    }
}

#[test]
fn retained_scope_projects_actual_declarations_and_queues_their_normal_destinations() {
    let runtime = CompileSession::from_source(
        r#"
VAR_GLOBAL RETAIN
    retained_global : INT := 2;
END_VAR
PROGRAM Main
VAR RETAIN
    retained_member : INT := 3;
END_VAR
END_PROGRAM
"#,
    )
    .build_runtime()
    .unwrap();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    let runtime_handle = adapter.session().runtime_handle();
    let control = adapter.session().debug_control();
    {
        let runtime = runtime_handle.lock().unwrap();
        assert!(
            runtime.storage().retain().is_empty(),
            "there is one ordinary storage authority"
        );
        control.refresh_snapshot_from_storage_with_abi(
            runtime.storage(),
            runtime.current_time(),
            runtime.prepared().storage_abi(),
        );
    }
    let scopes = adapter.dispatch_request(Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "scopes".into(),
        arguments: Some(serde_json::to_value(ScopesArguments { frame_id: 0 }).unwrap()),
    });
    let scopes: Response<ScopesResponseBody> =
        serde_json::from_value(scopes.responses[0].clone()).unwrap();
    let retained_handle = scopes
        .body
        .unwrap()
        .scopes
        .into_iter()
        .find(|scope| scope.name == "Retain")
        .unwrap()
        .variables_reference;
    let listing = adapter.dispatch_request(Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "variables".into(),
        arguments: Some(
            serde_json::to_value(VariablesArguments {
                variables_reference: retained_handle,
            })
            .unwrap(),
        ),
    });
    let listing: Response<VariablesResponseBody> =
        serde_json::from_value(listing.responses[0].clone()).unwrap();
    let variables = listing.body.unwrap().variables;
    assert!(variables
        .iter()
        .any(|variable| variable.name == "retained_global" && variable.value == "2"));
    assert!(variables
        .iter()
        .any(|variable| variable.name == "@program/Main/retained_member" && variable.value == "3"));
    for (seq, name, value) in [
        (3, "retained_global", "7"),
        (4, "@program/Main/retained_member", "8"),
    ] {
        let write = adapter.dispatch_request(Request {
            seq,
            message_type: MessageType::Request,
            command: "setVariable".into(),
            arguments: Some(
                serde_json::to_value(SetVariableArguments {
                    variables_reference: retained_handle,
                    name: name.into(),
                    value: value.into(),
                })
                .unwrap(),
            ),
        });
        let write: Response<SetVariableResponseBody> =
            serde_json::from_value(write.responses[0].clone()).unwrap();
        assert!(
            write.success,
            "retained declared destination queue failed: {write:?}"
        );
    }
    let mut runtime = runtime_handle.lock().unwrap();
    let Some(RuntimeValue::Instance(root)) = runtime.storage().get_global("Main") else {
        panic!("program root")
    };
    let root = *root;
    assert_eq!(
        runtime.storage().get_global("retained_global"),
        Some(&RuntimeValue::Int(2))
    );
    assert_eq!(
        runtime.storage().get_instance_var(root, "retained_member"),
        Some(&RuntimeValue::Int(3))
    );
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().get_global("retained_global"),
        Some(&RuntimeValue::Int(7))
    );
    assert_eq!(
        runtime.storage().get_instance_var(root, "retained_member"),
        Some(&RuntimeValue::Int(8))
    );
    assert!(runtime.storage().retain().is_empty());
    assert!(!runtime.faulted());
    assert!(!control
        .drain_runtime_events()
        .iter()
        .any(|event| matches!(event, RuntimeEvent::WriteRejected { .. })));
    drop(runtime);
    for (seq, name, value) in [
        (5, "retained_global", "force: 9"),
        (6, "@program/Main/retained_member", "force: 10"),
    ] {
        let force = adapter.dispatch_request(Request {
            seq,
            message_type: MessageType::Request,
            command: "setVariable".into(),
            arguments: Some(
                serde_json::to_value(SetVariableArguments {
                    variables_reference: retained_handle,
                    name: name.into(),
                    value: value.into(),
                })
                .unwrap(),
            ),
        });
        let force: Response<SetVariableResponseBody> =
            serde_json::from_value(force.responses[0].clone()).unwrap();
        assert!(force.success, "retained force failed: {force:?}");
    }
    let mut runtime = runtime_handle.lock().unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().get_global("retained_global"),
        Some(&RuntimeValue::Int(9))
    );
    assert_eq!(
        runtime.storage().get_instance_var(root, "retained_member"),
        Some(&RuntimeValue::Int(10))
    );
    runtime
        .write_global("retained_global", RuntimeValue::Int(0))
        .unwrap();
    let reference = runtime
        .storage()
        .ref_for_instance(root, "retained_member")
        .unwrap();
    runtime
        .write_reference(&reference, RuntimeValue::Int(0))
        .unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().get_global("retained_global"),
        Some(&RuntimeValue::Int(9)),
        "force persists on actual global storage"
    );
    assert_eq!(
        runtime.storage().get_instance_var(root, "retained_member"),
        Some(&RuntimeValue::Int(10)),
        "force persists on actual instance storage"
    );
    assert!(runtime.storage().retain().is_empty());
    assert!(!runtime.faulted());
}
