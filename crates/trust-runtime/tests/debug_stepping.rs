use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use trust_runtime::debug::{DebugBreakpoint, DebugStopReason, SourceLocation};
use trust_runtime::harness::{CompileSession, SourceFile};

fn line_index(source: &str, needle: &str) -> u32 {
    source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing line for {needle}")) as u32
}

fn resolve_location(
    runtime: &trust_runtime::Runtime,
    source: &str,
    file_id: u32,
    needle: &str,
) -> SourceLocation {
    let line = line_index(source, needle);
    runtime
        .resolve_breakpoint_location(source, file_id, line, 0)
        .unwrap_or_else(|| panic!("failed to resolve breakpoint for {needle}"))
}

#[test]
fn step_over_stops_in_caller_after_call() {
    let main = r#"PROGRAM Main
VAR
    Count : INT := 0;
END_VAR
    Count := AddTwo(Count);
    Count := Count + 1;
END_PROGRAM
"#;

    let lib = r#"FUNCTION AddTwo : INT
VAR_INPUT
    Value : INT;
END_VAR
    AddTwo := Value + 2;
END_FUNCTION
"#;

    let session = CompileSession::from_sources(vec![
        SourceFile::with_path("main.st", main),
        SourceFile::with_path("lib.st", lib),
    ]);
    let mut runtime = session.build_runtime().unwrap();
    let call_location = resolve_location(&runtime, main, 0, "Count := AddTwo");
    let expected_next = resolve_location(&runtime, main, 0, "Count := Count + 1");

    let control = runtime.enable_debug();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(call_location)]);

    let runtime = Arc::new(Mutex::new(runtime));
    let runtime_thread = runtime.clone();
    let handle = thread::spawn(move || {
        let mut runtime = runtime_thread.lock().expect("runtime lock poisoned");
        runtime.execute_cycle().unwrap();
    });

    let stop = stop_rx.recv_timeout(Duration::from_millis(500)).unwrap();
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);
    let thread_id = stop.thread_id.unwrap_or(1);

    control.step_over_thread(thread_id);
    let step_stop = stop_rx.recv_timeout(Duration::from_millis(500)).unwrap();
    assert_eq!(step_stop.reason, DebugStopReason::Step);
    let location = step_stop.location.expect("step location");
    assert_eq!(location.file_id, 0);
    assert_eq!(location.start, expected_next.start);

    control.continue_run();
    handle.join().unwrap();
}

#[test]
fn vm_breakpoint_populates_debug_snapshot_for_stack_queries() {
    let source = r#"PROGRAM Main
VAR
    Count : INT := 0;
END_VAR
    Count := Count + 1;
END_PROGRAM
"#;

    let session = CompileSession::from_sources(vec![SourceFile::new(source)]);
    let mut runtime = session.build_runtime().unwrap();
    let breakpoint_location = resolve_location(&runtime, source, 0, "Count := Count + 1");

    let control = runtime.enable_debug();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(breakpoint_location)]);

    let runtime = Arc::new(Mutex::new(runtime));
    let runtime_thread = runtime.clone();
    let handle = thread::spawn(move || {
        let mut runtime = runtime_thread.lock().expect("runtime lock poisoned");
        runtime.execute_cycle().unwrap();
    });

    let stop = stop_rx.recv_timeout(Duration::from_millis(500)).unwrap();
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);

    let snapshot = control
        .snapshot()
        .expect("expected paused snapshot at VM breakpoint");
    assert!(
        snapshot.storage.get_global("Main").is_some(),
        "expected snapshot to include runtime storage while paused"
    );

    control.continue_run();
    handle.join().unwrap();
}

#[test]
fn breakpoint_only_triggers_for_taken_branch() {
    let source = r#"PROGRAM Main
VAR
    Flag : BOOL := FALSE;
END_VAR
    IF Flag THEN
        Flag := TRUE;
    ELSE
        Flag := FALSE;
    END_IF;
END_PROGRAM
"#;

    let session = CompileSession::from_sources(vec![SourceFile::new(source)]);
    let mut runtime = session.build_runtime().unwrap();
    let if_location = resolve_location(&runtime, source, 0, "IF Flag THEN");
    let then_location = resolve_location(&runtime, source, 0, "Flag := TRUE");

    let control = runtime.enable_debug();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(then_location)]);

    let runtime = Arc::new(Mutex::new(runtime));
    let runtime_thread = runtime.clone();
    let handle = thread::spawn(move || {
        let mut runtime = runtime_thread.lock().expect("runtime lock poisoned");
        runtime.execute_cycle().unwrap();
    });

    let received = stop_rx.recv_timeout(Duration::from_millis(500));

    // Breakpoint matching can be precise (non-taken branch does not stop) or coarse when
    // statement ranges overlap (stop snaps to the enclosing IF location). Accept either behavior.
    // Always clean up the cycle thread before asserting.
    control.continue_run();
    handle.join().unwrap();

    if let Ok(stop) = received {
        assert_eq!(stop.reason, DebugStopReason::Breakpoint);
        let stop_location = stop.location.expect("stop location");
        assert_eq!(
            stop_location.start, if_location.start,
            "non-taken branch stop should only come from enclosing IF location"
        );
        assert!(
            stop_location.end >= then_location.end,
            "enclosing IF stop should cover the branch assignment range"
        );
    }
}

#[test]
fn breakpoint_triggers_for_executed_branch() {
    let source = r#"PROGRAM Main
VAR
    Flag : BOOL := TRUE;
END_VAR
    IF Flag THEN
        Flag := FALSE;
    ELSE
        Flag := TRUE;
    END_IF;
END_PROGRAM
"#;

    let session = CompileSession::from_sources(vec![SourceFile::new(source)]);
    let mut runtime = session.build_runtime().unwrap();
    let then_location = resolve_location(&runtime, source, 0, "Flag := FALSE");

    let control = runtime.enable_debug();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(then_location)]);

    let runtime = Arc::new(Mutex::new(runtime));
    let runtime_thread = runtime.clone();
    let handle = thread::spawn(move || {
        let mut runtime = runtime_thread.lock().expect("runtime lock poisoned");
        runtime.execute_cycle().unwrap();
    });

    let received = stop_rx.recv_timeout(Duration::from_millis(500));
    control.clear_breakpoints();
    control.continue_run();
    handle.join().unwrap();

    let stop = received.expect("expected breakpoint stop in taken branch");
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);
}

#[test]
fn breakpoint_rehits_each_cycle_after_continue() {
    let source = r#"PROGRAM Main
VAR
    StartCmd : BOOL := TRUE;
    Count : INT := 0;
END_VAR
    IF StartCmd THEN
        Count := Count + 1;
    END_IF;
END_PROGRAM
"#;

    let session = CompileSession::from_sources(vec![SourceFile::new(source)]);
    let mut runtime = session.build_runtime().unwrap();
    let if_location = resolve_location(&runtime, source, 0, "IF StartCmd THEN");

    let control = runtime.enable_debug();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);

    let runtime = Arc::new(Mutex::new(runtime));
    for cycle in 1..=3 {
        control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(if_location)]);
        let runtime_thread = runtime.clone();
        let handle = thread::spawn(move || {
            let mut runtime = runtime_thread.lock().expect("runtime lock poisoned");
            runtime.execute_cycle().unwrap();
        });

        let received = stop_rx.recv_timeout(Duration::from_millis(500));
        control.clear_breakpoints();
        control.continue_run();
        handle.join().unwrap();

        let stop = received.unwrap_or_else(|_| panic!("expected stop on cycle {cycle}"));
        assert_eq!(stop.reason, DebugStopReason::Breakpoint);
    }
}

#[test]
fn breakpoint_set_after_launch_hits_next_cycle() {
    let source = r#"PROGRAM Main
VAR
    Count : INT := 0;
END_VAR
    Count := Count + 1;
END_PROGRAM
"#;

    let session = CompileSession::from_sources(vec![SourceFile::new(source)]);
    let mut runtime = session.build_runtime().unwrap();
    let increment_location = resolve_location(&runtime, source, 0, "Count := Count + 1");

    let control = runtime.enable_debug();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);

    let (first_cycle_done_tx, first_cycle_done_rx) = channel();
    let (continue_cycles_tx, continue_cycles_rx) = channel();

    let runtime = Arc::new(Mutex::new(runtime));
    let runtime_thread = runtime.clone();
    let handle = thread::spawn(move || {
        let mut runtime = runtime_thread.lock().expect("runtime lock poisoned");

        // Launch/run starts with no breakpoints configured.
        runtime.execute_cycle().unwrap();
        first_cycle_done_tx.send(()).unwrap();

        // Simulate user adding a breakpoint after launch, then run next cycle.
        continue_cycles_rx.recv().unwrap();
        runtime.execute_cycle().unwrap();
    });

    first_cycle_done_rx
        .recv_timeout(Duration::from_millis(500))
        .unwrap();
    assert!(stop_rx.recv_timeout(Duration::from_millis(100)).is_err());

    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(increment_location)]);
    continue_cycles_tx.send(()).unwrap();

    let stop = stop_rx.recv_timeout(Duration::from_millis(500)).unwrap();
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);
    let location = stop.location.expect("breakpoint location");
    assert_eq!(location.file_id, 0);
    assert_eq!(location.start, increment_location.start);

    control.continue_run();
    handle.join().unwrap();
}

#[test]
fn breakpoint_set_while_running_hits_on_subsequent_cycle() {
    let source = r#"PROGRAM Main
VAR
    Count : DINT := 0;
END_VAR
    Count := Count + 1;
END_PROGRAM
"#;

    let session = CompileSession::from_sources(vec![SourceFile::new(source)]);
    let mut runtime = session.build_runtime().unwrap();
    let increment_location = resolve_location(&runtime, source, 0, "Count := Count + 1");

    let control = runtime.enable_debug();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);

    let running = Arc::new(AtomicBool::new(true));
    let runtime = Arc::new(Mutex::new(runtime));
    let runtime_thread = Arc::clone(&runtime);
    let running_thread = Arc::clone(&running);
    let handle = thread::spawn(move || {
        let mut runtime = runtime_thread.lock().expect("runtime lock poisoned");
        while running_thread.load(Ordering::SeqCst) {
            runtime.execute_cycle().unwrap();
        }
    });

    // Let launch/run proceed without breakpoints first.
    thread::sleep(Duration::from_millis(50));
    assert!(stop_rx.recv_timeout(Duration::from_millis(100)).is_err());

    // Set breakpoint while runtime is already running.
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(increment_location)]);

    let stop = stop_rx.recv_timeout(Duration::from_millis(1000)).unwrap();
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);
    let location = stop.location.expect("breakpoint location");
    assert_eq!(location.file_id, 0);
    assert_eq!(location.start, increment_location.start);

    control.continue_run();
    running.store(false, Ordering::SeqCst);
    handle.join().unwrap();
}
