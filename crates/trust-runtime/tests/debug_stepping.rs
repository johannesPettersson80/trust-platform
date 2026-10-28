//! Breakpoints and steps of a compiled program's debugger, through the control protocol's debug
//! operations (specification 13; specification 44 §13.2): a step over stops in the caller after
//! the call, a breakpoint stops only a statement that runs, stops again in each scan, and takes
//! effect when it is set after the program started.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value as Json};
use trust_runtime::debug::DebugControl;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::{CompiledDebugger, CompiledRuntime, RestartMode};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::ControlRequest;

/// The path of a single-file program's source, which the breakpoint requests name.
const MAIN: &str = "main.st";

/// The longest a test waits for a stop that must come.
const WAIT: Duration = Duration::from_secs(5);

fn line_index(source: &str, needle: &str) -> u64 {
    source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing line for {needle}")) as u64
}

/// The program of `files`, each `(path, text)`, built for this computer with debug hooks, loaded
/// and cold started, with its debug control and the debugger of the compiled program.
fn debugged(files: &[(&str, &str)]) -> (CompiledRuntime, DebugControl, CompiledDebugger) {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!(
        "trust-debug-stepping-{}-{:?}",
        std::process::id(),
        thread::current().id()
    ));
    let sources = files
        .iter()
        .map(|(path, text)| SourceFile::with_path(*path, *text))
        .collect::<Vec<_>>();
    let built = build(
        &sources,
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    let mut runtime = CompiledRuntime::load(built.image, "RESOURCE").expect("the image loads");
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    // A loaded program is not started (`CompiledRuntime::load`): it starts as trust-runtime starts
    // the program it loads, with a cold restart that gives every variable its starting value
    // (specification 44 §6.7), and its tasks count their activations from 0 ms.
    runtime.restart(RestartMode::Cold).expect("the cold start");
    runtime.reset_task_timing(trust_runtime::value::Duration::ZERO);
    let debugger = debug.compiled().expect("the compiled program's debugger");
    (runtime, debug, debugger)
}

/// The JSON of the debugger's answer to the debug operation `kind` with `params`.
fn ask(debugger: &CompiledDebugger, kind: &str, params: Option<Json>) -> Json {
    let request = ControlRequest {
        id: 1,
        r#type: kind.to_string(),
        params,
        auth: None,
        request_id: None,
    };
    serde_json::to_value(debugger.answer(&request, IoSnapshotRead::Missing))
        .expect("a response serializes")
}

/// Sets the breakpoints of `path` on the zero-based `lines`.
fn set_breakpoints(debugger: &CompiledDebugger, path: &str, lines: &[u64]) {
    let answer = ask(
        debugger,
        "breakpoints.set",
        Some(json!({ "source": path, "lines": lines })),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
}

/// Removes every breakpoint.
fn clear_breakpoints(debugger: &CompiledDebugger) {
    let answer = ask(debugger, "breakpoints.clear_all", None);
    assert_eq!(answer["ok"], json!(true), "{answer}");
}

/// The next stop the debugger records within `wait`, or `None`.
fn stop_within(debugger: &CompiledDebugger, wait: Duration) -> Option<Json> {
    let deadline = Instant::now() + wait;
    loop {
        let answer = ask(debugger, "debug.stops", None);
        if let Some(stop) = answer["result"]["stops"]
            .as_array()
            .and_then(|stops| stops.first())
        {
            return Some(stop.clone());
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(2));
    }
}

/// A scan of `runtime` on its own thread, as the resource's scan thread runs it; joining it gives
/// the runtime back.
fn scan(mut runtime: CompiledRuntime) -> thread::JoinHandle<CompiledRuntime> {
    thread::spawn(move || {
        runtime.execute_cycle().unwrap();
        runtime
    })
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

    let (runtime, _debug, debugger) = debugged(&[(MAIN, main), ("lib.st", lib)]);
    let call_line = line_index(main, "Count := AddTwo");
    let expected_next = line_index(main, "Count := Count + 1");
    set_breakpoints(&debugger, MAIN, &[call_line]);

    let handle = scan(runtime);

    let stop = stop_within(&debugger, WAIT).expect("the breakpoint stops");
    assert_eq!(stop["reason"], json!("breakpoint"), "{stop}");

    let step = ask(&debugger, "step_over", None);
    assert_eq!(step["ok"], json!(true), "{step}");
    let step_stop = stop_within(&debugger, WAIT).expect("the step stops");
    assert_eq!(step_stop["reason"], json!("step"), "{step_stop}");
    assert_eq!(step_stop["path"], json!(MAIN), "{step_stop}");
    assert_eq!(step_stop["line"], json!(expected_next), "{step_stop}");
    assert_eq!(step_stop["column"], json!(4), "{step_stop}");

    debugger.resume();
    handle.join().unwrap();
}

#[test]
fn breakpoint_populates_debug_snapshot_for_stack_queries() {
    let source = r#"PROGRAM Main
VAR
    Count : INT := 0;
END_VAR
    Count := Count + 1;
END_PROGRAM
"#;

    let (runtime, debug, debugger) = debugged(&[(MAIN, source)]);
    set_breakpoints(&debugger, MAIN, &[line_index(source, "Count := Count + 1")]);

    let handle = scan(runtime);

    let stop = stop_within(&debugger, WAIT).expect("the breakpoint stops");
    assert_eq!(stop["reason"], json!("breakpoint"), "{stop}");

    // The first scan is held before `Count := Count + 1` runs, so the held state holds 0.
    let snapshot = debug
        .snapshot()
        .expect("expected a published state at the breakpoint");
    assert_eq!(
        snapshot
            .compiled
            .as_ref()
            .and_then(|state| state.value("Main.Count")),
        Some(trust_runtime::value::Value::Int(0)),
        "expected the state to include the program's variables while paused"
    );
    let stack = ask(&debugger, "debug.stack", None);
    assert_eq!(stack["ok"], json!(true), "{stack}");

    debugger.resume();
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

    let (runtime, _debug, debugger) = debugged(&[(MAIN, source)]);
    set_breakpoints(&debugger, MAIN, &[line_index(source, "Flag := TRUE")]);

    let handle = scan(runtime);

    let received = stop_within(&debugger, Duration::from_millis(500));

    // The breakpoint is on the THEN branch, which does not run because Flag is FALSE. A statement
    // matches a breakpoint only when it starts inside the breakpoint's span (spec 13, hook
    // contract), and the enclosing IF starts before the branch, so no stop comes. Catches the old
    // overlap rule, which stopped at the enclosing IF. Always clean up the cycle thread first.
    debugger.resume();
    handle.join().unwrap();

    assert!(
        received.is_none(),
        "a breakpoint on a branch that does not run must not stop the scan, got {received:?}"
    );
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

    let (runtime, _debug, debugger) = debugged(&[(MAIN, source)]);
    let then_line = line_index(source, "Flag := FALSE");
    set_breakpoints(&debugger, MAIN, &[then_line]);

    let handle = scan(runtime);

    let received = stop_within(&debugger, WAIT);
    clear_breakpoints(&debugger);
    debugger.resume();
    handle.join().unwrap();

    // The branch runs because Flag is TRUE, so the breakpoint stops at the branch statement. The
    // enclosing IF starts before the branch and does not match it (spec 13, hook contract).
    let stop = received.expect("expected breakpoint stop in taken branch");
    assert_eq!(stop["reason"], json!("breakpoint"), "{stop}");
    assert_eq!(stop["line"], json!(then_line), "{stop}");
    assert_eq!(stop["column"], json!(8), "{stop}");
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

    let (mut runtime, _debug, debugger) = debugged(&[(MAIN, source)]);
    set_breakpoints(&debugger, MAIN, &[line_index(source, "IF StartCmd THEN")]);

    for cycle in 1..=3 {
        let handle = scan(runtime);

        let received = stop_within(&debugger, WAIT);
        debugger.resume();
        runtime = handle.join().unwrap();

        let stop = received.unwrap_or_else(|| panic!("expected stop on cycle {cycle}"));
        assert_eq!(stop["reason"], json!("breakpoint"), "{stop}");
    }
    clear_breakpoints(&debugger);
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

    let (mut runtime, _debug, debugger) = debugged(&[(MAIN, source)]);
    let increment_line = line_index(source, "Count := Count + 1");

    let (first_cycle_done_tx, first_cycle_done_rx) = channel();
    let (continue_cycles_tx, continue_cycles_rx) = channel();

    let handle = thread::spawn(move || {
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
    assert!(stop_within(&debugger, Duration::from_millis(100)).is_none());

    set_breakpoints(&debugger, MAIN, &[increment_line]);
    continue_cycles_tx.send(()).unwrap();

    let stop = stop_within(&debugger, WAIT).expect("the breakpoint stops the next cycle");
    assert_eq!(stop["reason"], json!("breakpoint"), "{stop}");
    assert_eq!(stop["path"], json!(MAIN), "{stop}");
    assert_eq!(stop["line"], json!(increment_line), "{stop}");
    assert_eq!(stop["column"], json!(4), "{stop}");

    debugger.resume();
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

    let (mut runtime, _debug, debugger) = debugged(&[(MAIN, source)]);
    let increment_line = line_index(source, "Count := Count + 1");

    let running = Arc::new(AtomicBool::new(true));
    let running_thread = Arc::clone(&running);
    let (first_cycle_done_tx, first_cycle_done_rx) = channel();
    let handle = thread::spawn(move || {
        let mut first_cycle_pending = true;
        while running_thread.load(Ordering::SeqCst) {
            runtime.execute_cycle().unwrap();
            if first_cycle_pending {
                first_cycle_done_tx.send(()).unwrap();
                first_cycle_pending = false;
            }
        }
    });

    // Prove launch/run completed a cycle without breakpoints before mutation.
    first_cycle_done_rx
        .recv_timeout(Duration::from_millis(500))
        .unwrap();
    assert!(stop_within(&debugger, Duration::from_millis(100)).is_none());

    // Set breakpoint while runtime is already running.
    set_breakpoints(&debugger, MAIN, &[increment_line]);

    let stop = stop_within(&debugger, WAIT).expect("the breakpoint stops a later cycle");
    assert_eq!(stop["reason"], json!("breakpoint"), "{stop}");
    assert_eq!(stop["path"], json!(MAIN), "{stop}");
    assert_eq!(stop["line"], json!(increment_line), "{stop}");
    assert_eq!(stop["column"], json!(4), "{stop}");

    running.store(false, Ordering::SeqCst);
    clear_breakpoints(&debugger);
    debugger.resume();
    handle.join().unwrap();
}
