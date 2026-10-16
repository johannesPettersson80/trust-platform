//! Paused hosted frames: physical slot order, scratch rows, DAP lists and watch retention.
use super::*;
use crate::protocol::VariablesResponseBody;
use trust_runtime::harness::{CompileSession, SourceFile};

/// Spec 34 HOST-09 (lines 473-474): compiler scratch slots stay resolvable by their
/// physical offsets, and the same frame's DAP variable list omits them.
#[test]
fn hosted_observation_scratch_rows_resolve_by_offset_and_stay_out_of_dap_lists() {
    let source = "FUNCTION Scan : DINT\nVAR_INPUT seed : DINT; END_VAR\nVAR i : DINT; END_VAR\nScan := seed;\nFOR i := DINT#1 TO DINT#5 BY DINT#2 DO\n    Scan := Scan + i;\nEND_FOR;\nEND_FUNCTION\nPROGRAM Main\nVAR result : DINT; END_VAR\nresult := Scan(DINT#3);\nEND_PROGRAM\n";
    let runtime = CompileSession::from_sources(vec![SourceFile::with_path("main.st", source)])
        .build_runtime()
        .unwrap();
    let mut session = DebugSession::new(runtime);
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);
    let control = adapter.session().debug_control();
    let line = source
        .lines()
        .position(|line| line.contains("Scan := Scan + i;"))
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
                    condition: None,
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
            .expect("breakpoint inside the FOR body");
        assert_eq!(stop.reason, DebugStopReason::Breakpoint);
        let snapshot = control.snapshot().expect("paused detached projection");
        let frame = snapshot
            .storage
            .frames()
            .iter()
            .find(|frame| frame.owner.eq_ignore_ascii_case("Scan"))
            .unwrap();
        // Declared slots come first in declaration order: return value, input, local.
        let declared: Vec<&str> = frame
            .variables
            .keys()
            .take(3)
            .map(|name| name.as_str())
            .collect();
        assert_eq!(declared, ["Scan", "seed", "i"]);
        // The two FOR temporaries (end, then step) occupy the physical slots after the
        // declared ones. Only their resolvability and omission are asserted here, as
        // spec 34 lines 473-474 require.
        let scratch: Vec<(usize, String)> = frame
            .variables
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| name.starts_with("@scratch/"))
            .map(|(offset, (name, _))| (offset, name.to_string()))
            .collect();
        assert_eq!(
            scratch,
            vec![(3, "@scratch/3".to_string()), (4, "@scratch/4".to_string())]
        );
        assert!(frame.variables.get("@scratch/3").is_some());
        assert_eq!(
            frame.variables.get_index(4).map(|(name, _)| name.as_str()),
            Some("@scratch/4")
        );

        let handle = adapter.alloc_variable_handle(VariableHandle::Locals(frame.id));
        let listed = adapter.dispatch_request(Request {
            seq: 2,
            message_type: MessageType::Request,
            command: "variables".into(),
            arguments: Some(serde_json::json!({ "variablesReference": handle })),
        });
        let listed: Response<VariablesResponseBody> =
            serde_json::from_value(listed.responses[0].clone()).unwrap();
        assert!(listed.success, "{listed:?}");
        let names: Vec<String> = listed
            .body
            .unwrap()
            .variables
            .into_iter()
            .map(|variable| variable.name)
            .collect();
        assert_eq!(names, ["Scan", "seed", "i"]);
    }));
    control.clear_breakpoints();
    control.continue_run();
    thread.join().unwrap().unwrap();
    if let Err(error) = inspected {
        std::panic::resume_unwind(error);
    }
}

/// Spec 34 HOST-09 (lines 469-471): paused frames keep local-slot ordering, so each
/// named row and its physical offset agree with the live invocation.
#[test]
fn hosted_observation_paused_frame_keeps_return_parameter_and_local_slot_order() {
    let source = "FUNCTION Leaf : INT\nVAR echo : INT; END_VAR\nLeaf := INT#1;\necho := Leaf;\nEND_FUNCTION\nPROGRAM Main\nVAR result : INT; END_VAR\nresult := Leaf();\nEND_PROGRAM\n";
    let runtime = CompileSession::from_sources(vec![SourceFile::with_path("main.st", source)])
        .build_runtime()
        .unwrap();
    let mut session = DebugSession::new(runtime);
    session.register_source("main.st", 0, source);
    let mut adapter = DebugAdapter::new(session);
    let control = adapter.session().debug_control();
    let line = source
        .lines()
        .position(|line| line.contains("echo := Leaf;"))
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
                    condition: None,
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

    let inspected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let stop = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("breakpoint after the return assignment");
        assert_eq!(stop.reason, DebugStopReason::Breakpoint);
        let snapshot = control.snapshot().expect("paused detached projection");
        let frame = snapshot
            .storage
            .frames()
            .iter()
            .find(|frame| frame.owner.eq_ignore_ascii_case("Leaf"))
            .unwrap();
        let names: Vec<&str> = frame.variables.keys().map(|name| name.as_str()).collect();
        assert_eq!(names, ["Leaf", "echo"]);
        assert_eq!(frame.variables.get_index_of("echo"), Some(1));
        assert_eq!(frame.variables.get("Leaf"), Some(&RuntimeValue::Int(1)));
        // The local has not been assigned yet, so its declared default is still visible.
        assert_eq!(frame.variables.get("echo"), Some(&RuntimeValue::Int(0)));
    }));
    control.clear_breakpoints();
    control.continue_run();
    thread.join().unwrap().unwrap();
    if let Err(error) = inspected {
        std::panic::resume_unwind(error);
    }
}

/// Spec 34 HOST-09 (lines 491-492): a failed reload preserves the installed watch
/// set, so the next cycle still evaluates the watch and reports its change.
#[test]
fn hosted_observation_failed_reload_keeps_the_installed_watch_set_for_the_next_cycle() {
    let root = std::env::temp_dir().join(format!(
        "trust-watch-failed-reload-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("main.st");
    let source =
        "VAR_GLOBAL count : DINT; END_VAR PROGRAM Main count := count + DINT#1; END_PROGRAM";
    std::fs::write(&path, source).unwrap();
    let runtime =
        CompileSession::from_sources(vec![SourceFile::with_path(path.to_string_lossy(), source)])
            .build_runtime()
            .unwrap();
    let mut session = DebugSession::new(runtime);
    session.register_source(path.to_string_lossy(), 0, source);
    session.set_program_path(path.to_string_lossy());
    let mut adapter = DebugAdapter::new(session);
    let outcome = adapter.dispatch_request(Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".into(),
        arguments: Some(serde_json::json!({"expression":"count","context":"watch"})),
    });
    let response: Response<EvaluateResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success, "{response:?}");
    assert_eq!(adapter.watch_cache.len(), 1);
    let control = adapter.session().debug_control();
    let runtime_handle = adapter.session().runtime_handle();
    runtime_handle.lock().unwrap().execute_cycle().unwrap();
    assert!(
        control.take_watch_changed(),
        "the registered watch is evaluated before the failed reload"
    );

    std::fs::write(&path, "PROGRAM Broken").unwrap();
    let outcome = adapter.dispatch_request(Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "stReload".into(),
        arguments: Some(serde_json::json!({"program": path})),
    });
    let reload: Response<serde_json::Value> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(!reload.success, "the broken candidate must be refused");
    assert_eq!(adapter.watch_cache.len(), 1);

    runtime_handle.lock().unwrap().execute_cycle().unwrap();
    assert!(
        control.take_watch_changed(),
        "the watch installed before the failed reload still runs in the next cycle"
    );
    std::fs::remove_dir_all(root).unwrap();
}
