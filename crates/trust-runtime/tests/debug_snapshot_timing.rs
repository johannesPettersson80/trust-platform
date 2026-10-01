//! When the debug snapshot is taken: docs/specs/13-debug-adapter.md (Breakpoint evaluation, source resolution, and hook
//! contract). A pause, step, or breakpoint stop snapshots the exact storage at that boundary; while running, the snapshot
//! shows the last completed cycle.

use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use trust_runtime::debug::{DebugBreakpoint, DebugSnapshot, DebugStopReason};
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::value::Value;

fn program_var(snapshot: &DebugSnapshot, name: &str) -> Value {
    let Some(Value::Instance(main)) = snapshot.storage.get_global("Main").cloned() else {
        panic!("the snapshot must hold the Main program instance");
    };
    snapshot
        .storage
        .get_instance_var(main, name)
        .cloned()
        .unwrap_or_else(|| panic!("the snapshot must hold {name}"))
}

#[test]
fn the_running_snapshot_shows_the_completed_cycle() {
    let source = "PROGRAM Main\nVAR\n    Count : DINT := 0;\nEND_VAR\n    Count := Count + 1;\nEND_PROGRAM\n";
    let session = CompileSession::from_sources(vec![SourceFile::new(source)]);
    let mut runtime = session.build_runtime().unwrap();
    let control = runtime.enable_debug();
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
    let session = CompileSession::from_sources(vec![SourceFile::new(source)]);
    let mut runtime = session.build_runtime().unwrap();
    let line = source
        .lines()
        .position(|line| line.contains("Second := 2"))
        .unwrap() as u32;
    let location = runtime
        .resolve_breakpoint_location(source, 0, line, 0)
        .expect("breakpoint location");
    let control = runtime.enable_debug();
    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(location)]);

    let runtime = Arc::new(Mutex::new(runtime));
    let cycle = runtime.clone();
    let handle = thread::spawn(move || cycle.lock().unwrap().execute_cycle().unwrap());

    let stop = stop_rx
        .recv_timeout(Duration::from_millis(2000))
        .expect("breakpoint stop");
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);
    let paused = control.snapshot().expect("a snapshot at the stop");
    assert_eq!(program_var(&paused, "First"), Value::DInt(1));
    assert_eq!(program_var(&paused, "Second"), Value::DInt(0));

    control.set_breakpoints_for_file(0, Vec::new());
    control.continue_run();
    handle.join().unwrap();
}
