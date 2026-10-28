//! When the debug snapshot is taken: docs/specs/13-debug-adapter.md (Breakpoint evaluation, source resolution, and hook
//! contract). A pause, step, or breakpoint stop snapshots the exact storage at that boundary; while running, the snapshot
//! shows the last completed cycle.

use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value as Json};
use trust_runtime::debug::{DebugControl, DebugSnapshot};
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::value::Value;
use trust_runtime::{CompiledDebugger, CompiledRuntime};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::ControlRequest;

const PATH: &str = "src/main.st";

/// The program of `source`, at [`PATH`], started cold with a debug control attached.
fn debugged(source: &str) -> (CompiledRuntime, DebugControl, CompiledDebugger) {
    let session = CompileSession::from_sources(vec![SourceFile::with_path(PATH, source)]);
    let mut runtime = TestHarness::from_session(session)
        .expect("the program compiles")
        .into_runtime();
    let control = DebugControl::new();
    runtime.set_debug_control(control.clone());
    let debugger = control.compiled().expect("the compiled program's debugger");
    (runtime, control, debugger)
}

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

/// The reason of the first recorded stop, waiting up to two seconds for it.
fn next_stop(debugger: &CompiledDebugger) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let answer = ask(debugger, "debug.stops", None);
        if let Some(stop) = answer["result"]["stops"]
            .as_array()
            .and_then(|stops| stops.first())
        {
            return stop["reason"].as_str().map(str::to_string);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(2));
    }
}

fn program_var(snapshot: &DebugSnapshot, name: &str) -> Value {
    let program = snapshot
        .compiled
        .as_deref()
        .expect("the snapshot holds the compiled program's state");
    program
        .value(&format!("Main.{name}"))
        .unwrap_or_else(|| panic!("the snapshot must hold {name}"))
}

#[test]
fn the_running_snapshot_shows_the_completed_cycle() {
    let source = "PROGRAM Main\nVAR\n    Count : DINT := 0;\nEND_VAR\n    Count := Count + 1;\nEND_PROGRAM\n";
    let (mut runtime, control, _debugger) = debugged(source);
    runtime.execute_cycle().unwrap();
    runtime.execute_cycle().unwrap();
    let snapshot = control
        .snapshot()
        .expect("a snapshot after a completed cycle");
    assert_eq!(program_var(&snapshot, "Count"), Value::DInt(2));
}

#[test]
fn a_breakpoint_stop_snapshots_the_storage_at_that_statement() {
    let source = "PROGRAM Main\nVAR\n    First : DINT := 0;\n    Second : DINT := 0;\nEND_VAR\n    First := 1;\n    Second := 2;\nEND_PROGRAM\n";
    let (mut runtime, control, debugger) = debugged(source);
    let line = source
        .lines()
        .position(|line| line.contains("Second := 2"))
        .unwrap();
    let answer = ask(
        &debugger,
        "breakpoints.set",
        Some(json!({ "source": PATH, "lines": [line] })),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");

    let handle = thread::spawn(move || runtime.execute_cycle().unwrap());

    assert_eq!(next_stop(&debugger).as_deref(), Some("breakpoint"));
    let paused = control.snapshot().expect("a snapshot at the stop");
    assert_eq!(program_var(&paused, "First"), Value::DInt(1));
    assert_eq!(program_var(&paused, "Second"), Value::DInt(0));

    let cleared = ask(
        &debugger,
        "breakpoints.set",
        Some(json!({ "source": PATH, "lines": [] })),
    );
    assert_eq!(cleared["ok"], json!(true), "{cleared}");
    debugger.resume();
    handle.join().unwrap();
}
