use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use trust_hir::types::TypeRegistry;
use trust_runtime::debug::{
    offset_to_line_col, resolve_breakpoint_location, DebugBreakpoint, DebugControl, DebugHook,
    DebugRuntimeContext, DebugStopReason, HitCondition, LogFragment, RuntimeEvent, SourceLocation,
};
use trust_runtime::harness::parse_debug_expression;
use trust_runtime::harness::TestHarness;
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::{DateTimeProfile, Value};
use trust_runtime::Runtime;

fn make_debug_context<'a>(
    storage: &'a mut VariableStorage,
    registry: &'a TypeRegistry,
) -> DebugRuntimeContext<'a> {
    DebugRuntimeContext {
        storage,
        registry,
        stdlib: None,
        profile: DateTimeProfile::default(),
        current_instance: None,
        now: trust_runtime::value::Duration::ZERO,
    }
}

fn wait_for_pause(control: &DebugControl, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if control.is_paused() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for debugger to enter paused mode");
}

#[test]
fn step_once_pauses_again() {
    let control = DebugControl::new();
    control.pause();

    let (tx, rx) = channel();
    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        hook.on_statement(Some(&SourceLocation::new(0, 0, 1)), 0);
        tx.send(1).unwrap();
        hook.on_statement(Some(&SourceLocation::new(0, 1, 2)), 0);
        tx.send(2).unwrap();
    });

    thread::sleep(Duration::from_millis(50));

    control.step();
    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 1);
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

    control.continue_run();
    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 2);

    handle.join().unwrap();
}

#[test]
fn breakpoint_pauses_execution() {
    let control = DebugControl::new();
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(SourceLocation::new(0, 0, 10))]);

    let (tx, rx) = channel();
    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        hook.on_statement(Some(&SourceLocation::new(0, 0, 10)), 0);
        tx.send(()).unwrap();
    });

    wait_for_pause(&control, Duration::from_secs(2));
    assert!(rx.try_recv().is_err());

    control.continue_run();
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    handle.join().unwrap();
}

#[test]
fn step_over_pauses_at_same_depth() {
    let control = DebugControl::new();
    control.pause();

    let (tx, rx) = channel();
    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        hook.on_statement(Some(&SourceLocation::new(0, 0, 1)), 0);
        tx.send(1).unwrap();
        hook.on_statement(Some(&SourceLocation::new(0, 2, 3)), 1);
        tx.send(2).unwrap();
        hook.on_statement(Some(&SourceLocation::new(0, 4, 5)), 0);
        tx.send(3).unwrap();
    });

    thread::sleep(Duration::from_millis(50));
    control.step_over();

    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 1);
    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 2);
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

    control.continue_run();
    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 3);
    handle.join().unwrap();
}

#[test]
fn step_out_pauses_after_return() {
    let control = DebugControl::new();
    control.pause();

    let (tx, rx) = channel();
    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        hook.on_statement(Some(&SourceLocation::new(0, 0, 1)), 1);
        tx.send(1).unwrap();
        hook.on_statement(Some(&SourceLocation::new(0, 2, 3)), 2);
        tx.send(2).unwrap();
        hook.on_statement(Some(&SourceLocation::new(0, 4, 5)), 1);
        tx.send(3).unwrap();
        hook.on_statement(Some(&SourceLocation::new(0, 6, 7)), 0);
        tx.send(4).unwrap();
    });

    thread::sleep(Duration::from_millis(50));
    control.step_out();

    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 1);
    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 2);
    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 3);
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

    control.continue_run();
    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 4);
    handle.join().unwrap();
}

#[test]
fn resolve_breakpoint_prefers_inner_statement() {
    let source = "IF x THEN\n  a := 1;\nEND_IF;\n";
    let outer_start = source.find("IF").unwrap();
    let outer_end = source.find("END_IF;").unwrap() + "END_IF;".len();
    let inner_start = source.find("a := 1;").unwrap();
    let inner_end = inner_start + "a := 1;".len();

    let statements = vec![
        SourceLocation::new(0, outer_start as u32, outer_end as u32),
        SourceLocation::new(0, inner_start as u32, inner_end as u32),
    ];

    let resolved = resolve_breakpoint_location(source, 0, &statements, 1, 2).unwrap();
    assert_eq!(resolved.start, inner_start as u32);
}

#[test]
fn resolve_breakpoint_next_statement() {
    let source = "a := 1;\n\nb := 2;\n";
    let a_start = source.find("a := 1;").unwrap();
    let a_end = a_start + "a := 1;".len();
    let b_start = source.find("b := 2;").unwrap();
    let b_end = b_start + "b := 2;".len();

    let statements = vec![
        SourceLocation::new(0, a_start as u32, a_end as u32),
        SourceLocation::new(0, b_start as u32, b_end as u32),
    ];

    let resolved = resolve_breakpoint_location(source, 0, &statements, 1, 0).unwrap();
    assert_eq!(resolved.start, b_start as u32);
}

#[test]
fn runtime_resolves_breakpoint_using_index() {
    let mut runtime = Runtime::new();
    let source = "x := 1;\ny := 2;\n";
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

    let resolved = runtime
        .resolve_breakpoint_location(source, 0, 1, 0)
        .unwrap();
    assert_eq!(resolved.start, y_start as u32);
}

#[test]
fn runtime_resolves_breakpoint_position_to_statement_start() {
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

    let (location, line, col) = runtime
        .resolve_breakpoint_position(source, 0, 1, 0)
        .unwrap();
    assert_eq!(location.start, y_start as u32);
    assert_eq!(line, 1);
    assert_eq!(col, 2);
}

#[test]
fn statement_locations_use_first_token_in_if_block() {
    let source = r#"FUNCTION AddTwo : INT
VAR_INPUT
    x : INT;
END_VAR
    AddTwo := x + 2;
END_FUNCTION

PROGRAM Main
VAR
    InSignal : BOOL := FALSE;
    OutSignal : BOOL := FALSE;
    Count : INT := 0;
    Count2 : INT := 0;
END_VAR

Count2 := 1;
IF InSignal THEN
    Count := AddTwo(Count);
END_IF;
OutSignal := (Count MOD 2) = 1;
END_PROGRAM
"#;
    let harness = TestHarness::from_source(source).unwrap();
    let runtime = harness.into_runtime();
    let locations = runtime.statement_locations(0).expect("statement locations");
    let target_line = source
        .lines()
        .position(|line| line.contains("Count := AddTwo"))
        .unwrap();
    let target_col = source
        .lines()
        .nth(target_line)
        .unwrap()
        .find("Count := AddTwo")
        .unwrap();
    let found = locations.iter().any(|loc| {
        let (line, col) = offset_to_line_col(source, loc.start);
        line == target_line as u32 && col == target_col as u32
    });
    assert!(found, "expected statement location at Count := AddTwo");
}

#[test]
fn breakpoint_emits_stop_event() {
    let control = DebugControl::new();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(SourceLocation::new(0, 0, 1))]);

    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        hook.on_statement(Some(&SourceLocation::new(0, 0, 1)), 0);
    });

    let stop = stop_rx.recv_timeout(Duration::from_millis(250)).unwrap();
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);
    assert_eq!(stop.breakpoint_generation, control.breakpoint_generation(0));
    control.continue_run();
    handle.join().unwrap();
}

#[test]
fn breakpoint_generation_increments_on_clear() {
    let control = DebugControl::new();
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(SourceLocation::new(0, 0, 1))]);
    let gen_before = control.breakpoint_generation(0).unwrap();
    control.set_breakpoints_for_file(0, Vec::new());
    let gen_after = control.breakpoint_generation(0).unwrap();
    assert!(gen_after > gen_before);
}

#[test]
fn frame_location_tracks_current_frame() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 5, 10);
    let mut storage = VariableStorage::new();
    let frame_id = storage.push_frame("MAIN");
    let registry = TypeRegistry::new();
    let mut ctx = make_debug_context(&mut storage, &registry);

    let mut hook = control.clone();
    hook.on_statement_with_context(&mut ctx, Some(&location), 0);

    assert_eq!(control.frame_location(frame_id), Some(location));
}

#[test]
fn watch_changes_reported_between_stops() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 0, 1);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(location)]);
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);

    let mut registry = TypeRegistry::new();
    let watch =
        parse_debug_expression("x", &mut registry, DateTimeProfile::default(), &[]).unwrap();
    control.register_watch_expression(watch);

    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        let mut storage = VariableStorage::new();
        storage.push_frame("MAIN");
        storage.set_local("x", Value::DInt(1));
        let registry = TypeRegistry::new();
        let mut ctx = make_debug_context(&mut storage, &registry);
        hook.on_statement_with_context(&mut ctx, Some(&location), 0);
        ctx.storage.set_local("x", Value::DInt(2));
        hook.on_statement_with_context(&mut ctx, Some(&location), 0);
    });

    stop_rx.recv_timeout(Duration::from_millis(250)).unwrap();
    assert!(control.take_watch_changed());
    control.continue_run();

    stop_rx.recv_timeout(Duration::from_millis(250)).unwrap();
    assert!(control.take_watch_changed());
    control.continue_run();

    handle.join().unwrap();
}

#[test]
fn pause_preserves_task_order() {
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

    let mut harness = trust_runtime::harness::TestHarness::from_source(source).unwrap();
    harness.set_input("trigger1", Value::Bool(true));
    harness.set_input("trigger2", Value::Bool(true));
    let control = harness.runtime_mut().enable_debug();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);

    let line = source
        .lines()
        .position(|line| line.contains("trace := trace * INT#10 + INT#1;"))
        .unwrap() as u32;
    let location = harness
        .runtime()
        .resolve_breakpoint_location(source, 0, line, 0)
        .unwrap();
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(location)]);

    let runtime = std::sync::Arc::new(std::sync::Mutex::new(harness.into_runtime()));
    let runtime_thread = runtime.clone();
    let handle = thread::spawn(move || {
        let mut runtime = runtime_thread.lock().expect("runtime lock poisoned");
        runtime.execute_cycle().unwrap();
    });

    stop_rx.recv_timeout(Duration::from_millis(250)).unwrap();
    control.continue_run();
    handle.join().unwrap();

    let runtime = runtime.lock().expect("runtime lock poisoned");
    assert_eq!(runtime.storage().get_global("trace"), Some(&Value::Int(12)));
}

#[test]
fn conditional_breakpoint_skips_when_false() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 0, 1);

    let mut registry = TypeRegistry::new();
    let condition =
        parse_debug_expression("x > 0", &mut registry, DateTimeProfile::default(), &[]).unwrap();

    control.set_breakpoints_for_file(
        0,
        vec![DebugBreakpoint {
            location,
            condition: Some(condition),
            hit_condition: None,
            log_message: None,
            hits: 0,
            generation: 0,
        }],
    );

    let (tx, rx) = channel();
    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        let mut storage = VariableStorage::new();
        storage.push_frame("MAIN");
        storage.set_local("x", Value::DInt(0));
        let registry = TypeRegistry::new();
        let mut ctx = make_debug_context(&mut storage, &registry);
        hook.on_statement_with_context(&mut ctx, Some(&location), 0);
        tx.send(()).unwrap();
    });

    rx.recv_timeout(Duration::from_millis(250)).unwrap();
    handle.join().unwrap();
}

#[test]
fn conditional_breakpoint_pauses_when_true() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 0, 1);

    let mut registry = TypeRegistry::new();
    let condition =
        parse_debug_expression("x > 0", &mut registry, DateTimeProfile::default(), &[]).unwrap();

    control.set_breakpoints_for_file(
        0,
        vec![DebugBreakpoint {
            location,
            condition: Some(condition),
            hit_condition: None,
            log_message: None,
            hits: 0,
            generation: 0,
        }],
    );

    let (tx, rx) = channel();
    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        let mut storage = VariableStorage::new();
        storage.push_frame("MAIN");
        storage.set_local("x", Value::DInt(1));
        let registry = TypeRegistry::new();
        let mut ctx = make_debug_context(&mut storage, &registry);
        hook.on_statement_with_context(&mut ctx, Some(&location), 0);
        tx.send(()).unwrap();
    });

    wait_for_pause(&control, Duration::from_secs(2));
    assert!(rx.try_recv().is_err());

    control.continue_run();
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    handle.join().unwrap();
}

#[test]
fn hit_count_breakpoint_pauses_on_threshold() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 0, 1);

    control.set_breakpoints_for_file(
        0,
        vec![DebugBreakpoint {
            location,
            condition: None,
            hit_condition: Some(HitCondition::Equal(3)),
            log_message: None,
            hits: 0,
            generation: 0,
        }],
    );

    let (tx, rx) = channel();
    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        let mut storage = VariableStorage::new();
        storage.push_frame("MAIN");
        let registry = TypeRegistry::new();
        let mut ctx = make_debug_context(&mut storage, &registry);

        hook.on_statement_with_context(&mut ctx, Some(&location), 0);
        tx.send(1).unwrap();
        hook.on_statement_with_context(&mut ctx, Some(&location), 0);
        tx.send(2).unwrap();
        hook.on_statement_with_context(&mut ctx, Some(&location), 0);
        tx.send(3).unwrap();
    });

    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 1);
    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 2);
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

    control.continue_run();
    assert_eq!(rx.recv_timeout(Duration::from_millis(250)).unwrap(), 3);
    handle.join().unwrap();
}

#[test]
fn logpoint_emits_output_without_pausing() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 0, 1);

    let mut registry = TypeRegistry::new();
    let expr = parse_debug_expression("x", &mut registry, DateTimeProfile::default(), &[]).unwrap();

    control.set_breakpoints_for_file(
        0,
        vec![DebugBreakpoint {
            location,
            condition: None,
            hit_condition: None,
            log_message: Some(vec![
                LogFragment::Text("x=".to_string()),
                LogFragment::Expr(expr),
            ]),
            hits: 0,
            generation: 0,
        }],
    );

    let (tx, rx) = channel();
    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        let mut storage = VariableStorage::new();
        storage.push_frame("MAIN");
        storage.set_local("x", Value::DInt(41));
        let registry = TypeRegistry::new();
        let mut ctx = make_debug_context(&mut storage, &registry);
        hook.on_statement_with_context(&mut ctx, Some(&location), 0);
        tx.send(()).unwrap();
    });

    rx.recv_timeout(Duration::from_millis(250)).unwrap();
    handle.join().unwrap();

    let logs = control.drain_logs();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].message.contains("x=DInt(41)"));
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 8"]
fn runtime_event_sender_drop_buffers_event_in_debug_control() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    drop(rx);
    control.set_runtime_sender(tx);

    control.push_runtime_event(RuntimeEvent::Fault {
        error: "first fault".to_string(),
        time: trust_runtime::value::Duration::ZERO,
    });

    let events = control.drain_runtime_events();
    assert!(
        events.iter().any(
            |event| matches!(event, RuntimeEvent::Fault { error, .. } if error == "first fault")
        ),
        "dropped runtime event sender must not lose the first fault, got {events:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 8"]
fn logpoint_sender_drop_buffers_log_in_debug_control() {
    let control = DebugControl::new();
    let (log_tx, log_rx) = channel();
    drop(log_rx);
    control.set_log_sender(log_tx);
    let location = SourceLocation::new(0, 0, 1);

    control.set_breakpoints_for_file(
        0,
        vec![DebugBreakpoint {
            location,
            condition: None,
            hit_condition: None,
            log_message: Some(vec![LogFragment::Text("fault log".to_string())]),
            hits: 0,
            generation: 0,
        }],
    );

    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        let mut storage = VariableStorage::new();
        storage.push_frame("MAIN");
        let registry = TypeRegistry::new();
        let mut ctx = make_debug_context(&mut storage, &registry);
        hook.on_statement_with_context(&mut ctx, Some(&location), 0);
    });
    handle.join().unwrap();

    let logs = control.drain_logs();
    assert_eq!(
        logs.first().map(|log| log.message.as_str()),
        Some("fault log"),
        "dropped log sender must fall back to the in-memory debug log buffer"
    );
}
