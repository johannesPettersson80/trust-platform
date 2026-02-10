//! Adapter unit tests.
//! - stdio framing roundtrips
//! - request dispatch smoke tests

use super::protocol_io::{read_message, write_message};
use super::*;
use crate::protocol::{
    BreakpointLocationsArguments, BreakpointLocationsResponseBody, ContinueArguments,
    EvaluateArguments, EvaluateResponseBody, Event, InitializeArguments, InitializeResponseBody,
    IoStateEventBody, IoWriteArguments, MessageType, NextArguments, PauseArguments, Request,
    Response, ScopesArguments, ScopesResponseBody, SetBreakpointsArguments,
    SetBreakpointsResponseBody, SetExpressionArguments, SetExpressionResponseBody, Source,
    SourceBreakpoint, StackTraceArguments, StackTraceResponseBody, StepInArguments,
    StepOutArguments, ThreadsResponseBody, VariablesArguments, VariablesResponseBody,
};
use crate::DebugSession;
use indexmap::IndexMap;
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::io::BufReader;
use trust_hir::{Type, TypeId};
use trust_runtime::debug::{DebugControl, DebugHook, DebugStopReason, SourceLocation};
use trust_runtime::harness::TestHarness;
use trust_runtime::io::IoAddress;
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::Value as RuntimeValue;
use trust_runtime::value::{ArrayValue, Duration, StructValue};
use trust_runtime::Runtime;

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
    runtime.io_mut().bind("IN0", input_addr.clone());
    runtime.io_mut().bind("OUT0", output_addr.clone());
    runtime
        .io_mut()
        .write(&input_addr, RuntimeValue::Bool(true))
        .unwrap();
    runtime
        .io_mut()
        .write(&output_addr, RuntimeValue::Bool(false))
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
    assert_eq!(outcome.events.len(), 1);
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert_eq!(event.event, "stIoState");

    let value = adapter
        .session()
        .runtime_handle()
        .lock()
        .unwrap()
        .io()
        .read(&input_addr)
        .unwrap();
    assert_eq!(value, RuntimeValue::Bool(true));
}

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
        counter <= 6,
        "expected interval pacing to cap cycle count, got Counter={counter}"
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
        RuntimeValue::Struct(StructValue {
            type_name: SmolStr::new("MY_STRUCT"),
            fields,
        }),
    );
    runtime.storage_mut().set_local(
        "arr",
        RuntimeValue::Array(ArrayValue {
            elements: vec![RuntimeValue::Int(1), RuntimeValue::Int(2)],
            dimensions: vec![(1, 2)],
        }),
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
fn dispatch_evaluate_returns_value() {
    let mut runtime = Runtime::new();
    let frame_id = runtime.storage_mut().push_frame("MAIN");
    runtime
        .storage_mut()
        .set_local("foo", RuntimeValue::Int(41));

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "foo + 1".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(eval_req);
    let response: Response<EvaluateResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success);
    let body = response.body.unwrap();
    assert_eq!(body.result, "DInt(42)");
}

#[test]
fn dispatch_evaluate_rejects_calls() {
    let runtime = Runtime::new();
    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "foo()".to_string(),
                frame_id: None,
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(eval_req);
    let response: Response<Value> = serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(!response.success);
}

#[test]
fn dispatch_evaluate_allows_pure_stdlib_calls() {
    let mut runtime = Runtime::new();
    let frame_id = runtime.storage_mut().push_frame("MAIN");

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "ABS(-1)".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(eval_req);
    let response: Response<EvaluateResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success);
    assert_eq!(response.body.unwrap().result, "DInt(1)");
}

#[test]
fn dispatch_evaluate_resolves_instance_and_retain() {
    let mut runtime = Runtime::new();
    let instance_id = runtime.storage_mut().create_instance("MyFB");
    runtime
        .storage_mut()
        .set_instance_var(instance_id, "iv", RuntimeValue::Int(7));
    let frame_id = runtime
        .storage_mut()
        .push_frame_with_instance("METHOD", instance_id);
    runtime.storage_mut().set_retain("r", RuntimeValue::DInt(9));

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_instance_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "iv".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let instance_outcome = adapter.dispatch_request(eval_instance_req);
    let instance_response: Response<EvaluateResponseBody> =
        serde_json::from_value(instance_outcome.responses[0].clone()).unwrap();
    assert!(instance_response.success);
    assert_eq!(instance_response.body.unwrap().result, "Int(7)");

    let eval_this_req = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "THIS".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let this_outcome = adapter.dispatch_request(eval_this_req);
    let this_response: Response<EvaluateResponseBody> =
        serde_json::from_value(this_outcome.responses[0].clone()).unwrap();
    assert!(this_response.success);
    assert_eq!(
        this_response.body.unwrap().result,
        format!("Instance({})", instance_id.0)
    );

    let eval_retain_req = Request {
        seq: 3,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "r".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let retain_outcome = adapter.dispatch_request(eval_retain_req);
    let retain_response: Response<EvaluateResponseBody> =
        serde_json::from_value(retain_outcome.responses[0].clone()).unwrap();
    assert!(retain_response.success);
    assert_eq!(retain_response.body.unwrap().result, "DInt(9)");
}

#[test]
fn dispatch_evaluate_honors_using_for_types() {
    let mut runtime = Runtime::new();
    let type_name = SmolStr::new("UTIL.MYINT");
    runtime.registry_mut().register(
        type_name.clone(),
        Type::Alias {
            name: type_name,
            target: TypeId::INT,
        },
    );
    runtime
        .register_program(ProgramDef {
            name: SmolStr::new("MAIN"),
            vars: Vec::new(),
            temps: Vec::new(),
            using: vec![SmolStr::new("UTIL")],
            body: Vec::new(),
        })
        .unwrap();
    let frame_id = runtime.storage_mut().push_frame("MAIN");

    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);

    let eval_req = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "evaluate".to_string(),
        arguments: Some(
            serde_json::to_value(EvaluateArguments {
                expression: "SIZEOF(MYINT)".to_string(),
                frame_id: Some(frame_id.0),
                context: Some("watch".to_string()),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(eval_req);
    let response: Response<EvaluateResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success);
    assert_eq!(response.body.unwrap().result, "DInt(2)");
}
