use super::*;

#[test]
fn dispatch_pause_falls_back_to_global_when_no_active_thread() {
    let runtime = Runtime::new();
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    let control = adapter.session().debug_control();

    control.set_current_thread(None);
    let pause_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "pause".to_string(),
        arguments: Some(serde_json::to_value(PauseArguments { thread_id: 1 }).unwrap()),
    };

    adapter.dispatch_request(pause_req);
    assert_eq!(control.mode(), trust_runtime::debug::DebugMode::Paused);
    assert_eq!(control.target_thread(), None);
}

#[test]
fn dispatch_continue_then_immediate_pause_emits_pause_stop() {
    let source = r#"PROGRAM Main
VAR
    x : INT := 0;
END_VAR
x := x + 1;
END_PROGRAM
"#;
    let harness = TestHarness::from_source(source).unwrap();
    let mut session = DebugSession::new(harness.into_runtime());
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);
    let control = adapter.session().debug_control();

    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    control.set_stop_sender(stop_tx);

    let line = source
        .lines()
        .position(|line| line.contains("x := x + 1;"))
        .unwrap() as u32
        + 1;
    let set_breakpoint_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setBreakpoints".to_string(),
        arguments: Some(
            serde_json::to_value(SetBreakpointsArguments {
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
            })
            .unwrap(),
        ),
    };
    let _ = adapter.dispatch_request(set_breakpoint_req);

    let runtime = adapter.session().runtime_handle();
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag_thread = std::sync::Arc::clone(&stop_flag);
    let handle = std::thread::spawn(move || {
        while !stop_flag_thread.load(std::sync::atomic::Ordering::Relaxed) {
            let mut guard = runtime.lock().unwrap();
            let _ = guard.execute_cycle();
        }
    });

    let first = stop_rx
        .recv_timeout(std::time::Duration::from_millis(250))
        .expect("first stop");
    assert_eq!(first.reason, DebugStopReason::Breakpoint);

    let clear_breakpoint_req = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "setBreakpoints".to_string(),
        arguments: Some(
            serde_json::to_value(SetBreakpointsArguments {
                source: Source {
                    name: Some("main".into()),
                    path: Some("main.st".into()),
                    source_reference: None,
                },
                breakpoints: Some(Vec::new()),
                lines: None,
                source_modified: None,
            })
            .unwrap(),
        ),
    };
    let _ = adapter.dispatch_request(clear_breakpoint_req);

    let continue_req = Request {
        seq: 3,
        message_type: MessageType::Request,
        command: "continue".to_string(),
        arguments: Some(serde_json::to_value(ContinueArguments { thread_id: 1 }).unwrap()),
    };
    let _ = adapter.dispatch_request(continue_req);

    let pause_req = Request {
        seq: 4,
        message_type: MessageType::Request,
        command: "pause".to_string(),
        arguments: Some(serde_json::to_value(PauseArguments { thread_id: 1 }).unwrap()),
    };
    let _ = adapter.dispatch_request(pause_req);

    let second = stop_rx
        .recv_timeout(std::time::Duration::from_millis(250))
        .expect("pause stop");
    assert_eq!(second.reason, DebugStopReason::Pause);

    stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    control.continue_run();
    handle.join().expect("hook thread joins");
}

#[test]
fn dispatch_threads_maps_tasks() {
    let mut runtime = Runtime::new();
    runtime
        .register_program(ProgramDef {
            name: SmolStr::new("MAIN"),
            vars: Vec::new(),
            temps: Vec::new(),
            using: Vec::new(),
            body: Vec::new(),
        })
        .unwrap();
    runtime.register_task(TaskConfig {
        name: SmolStr::new("FAST"),
        interval: Duration::ZERO,
        single: None,
        priority: 1,
        programs: Vec::new(),
        fb_instances: Vec::new(),
    });
    runtime.register_task(TaskConfig {
        name: SmolStr::new("SLOW"),
        interval: Duration::ZERO,
        single: None,
        priority: 2,
        programs: Vec::new(),
        fb_instances: Vec::new(),
    });

    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    let threads_req = Request::<serde_json::Value> {
        seq: 1,
        message_type: MessageType::Request,
        command: "threads".to_string(),
        arguments: None,
    };
    let threads_outcome = adapter.dispatch_request(threads_req);
    let threads_response: Response<ThreadsResponseBody> =
        serde_json::from_value(threads_outcome.responses[0].clone()).unwrap();
    let threads = threads_response.body.unwrap().threads;
    assert_eq!(threads.len(), 3);
    assert_eq!(threads[0].id, 1);
    assert_eq!(threads[0].name, "FAST");
    assert_eq!(threads[1].id, 2);
    assert_eq!(threads[1].name, "SLOW");
    assert_eq!(threads[2].id, 3);
    assert_eq!(threads[2].name, "Background");
}

#[test]
fn debug_runner_respects_task_interval_pacing() {
    let source = r#"
CONFIGURATION Conf
VAR_GLOBAL
    Counter : DINT := DINT#0;
END_VAR
TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM P1 WITH MainTask : MainProg;
END_CONFIGURATION

PROGRAM MainProg
Counter := Counter + DINT#1;
END_PROGRAM
"#;

    let harness = TestHarness::from_source(source).expect("compile source");
    let session = DebugSession::new(harness.into_runtime());
    let mut adapter = DebugAdapter::new(session);

    adapter.start_runner();
    std::thread::sleep(std::time::Duration::from_millis(240));
    adapter.stop_runner();

    let runtime = adapter.session().runtime_handle();
    let guard = runtime.lock().expect("runtime lock");
    let counter = match guard.storage().get_global("Counter") {
        Some(RuntimeValue::DInt(value)) => *value,
        other => panic!("unexpected Counter value: {other:?}"),
    };

    assert!(
        (1..=6).contains(&counter),
        "expected the runner to execute while interval pacing caps the cycle count, got Counter={counter}"
    );
}
