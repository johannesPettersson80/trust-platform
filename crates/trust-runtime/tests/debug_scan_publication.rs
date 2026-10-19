//! Debugger reads and storage copies of a hosted scan (specification 11 section 6.13.1, section 6.9;
//! specification 13 "Variables / Evaluate").
//!
//! Expected values are worked out by hand from the programs below. `n` starts at 0 and the program
//! of `SOURCE` adds 1, then 10, then sets `lamp`, so the first scan ends with `n = 11` and the second
//! with `n = 22`. A stop before the second statement of the second scan holds `n = 12`.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use trust_runtime::debug::{DebugBreakpoint, DebugControl, DebugStopReason, SourceLocation};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::CompileSession;
use trust_runtime::io::IoDriver;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

const WAIT: Duration = Duration::from_secs(5);

const SOURCE: &str = r#"
VAR_GLOBAL
    n : DINT := DINT#0;
    lamp AT %QX0.0 : BOOL := FALSE;
END_VAR

PROGRAM Main
n := n + DINT#1;
n := n + DINT#10;
lamp := n > DINT#0;
END_PROGRAM
"#;

/// The first scan divides by one, and the second divides by zero after it has stored `n = 2`.
const FAULT_SOURCE: &str = r#"
VAR_GLOBAL
    n : DINT := DINT#0;
    ratio : DINT := DINT#0;
END_VAR

PROGRAM Main
n := n + DINT#1;
ratio := DINT#10 / (DINT#2 - n);
END_PROGRAM
"#;

/// An output driver that reports each output image and then waits for the test to release it. A
/// scan waiting here has run every statement but has not completed.
struct GateDriver {
    entered: Sender<()>,
    release: Receiver<()>,
}

impl IoDriver for GateDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        // A closed channel releases the scan too, so a failed assertion never leaves it waiting.
        let _ = self.entered.send(());
        let _ = self.release.recv();
        Ok(())
    }
}

fn build(source: &str) -> Runtime {
    CompileSession::from_source(source)
        .build_runtime()
        .expect("the fixture compiles")
}

/// The location of the statement on the line that contains `needle`.
fn statement_location(runtime: &Runtime, source: &str, needle: &str) -> SourceLocation {
    let line = source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing line for {needle}")) as u32;
    runtime
        .resolve_breakpoint_location(source, 0, line, 0)
        .unwrap_or_else(|| panic!("no statement on the line of {needle}"))
}

/// The value of a global in the state reads answer from, if a state is published.
fn published_dint(debug: &DebugControl, name: &str) -> Option<Value> {
    debug
        .snapshot()
        .and_then(|snapshot| snapshot.storage.get_global(name).cloned())
}

#[test]
fn a_scan_with_debugging_enabled_copies_once_at_its_end() {
    // Spec 11 section 6.13.1, bullet 2: one published state per completed scan, made at its end.
    // Wrong behaviour caught: a copy per located statement (three per scan here) or per instruction
    // (more than three per scan here).
    let mut runtime = build(SOURCE);
    let debug = runtime.enable_debug();

    runtime.execute_cycle().expect("the first scan completes");
    assert_eq!(
        debug.storage_copies(),
        1,
        "one completed scan makes one copy, at its end"
    );

    runtime.execute_cycle().expect("the second scan completes");
    assert_eq!(
        debug.storage_copies(),
        2,
        "two completed scans make two copies"
    );
    assert_eq!(
        runtime.storage().get_global("n"),
        Some(&Value::DInt(22)),
        "the program itself is unchanged by the observation"
    );
}

#[test]
fn a_scan_with_debugging_disabled_copies_no_storage() {
    // Spec 11 section 6.9: debug hooks are side-effect-free when disabled, and a config.set of
    // runtime.control.debug_enabled takes effect as the setting already does. Wrong behaviour caught:
    // the hook copying storage or holding the scan at a breakpoint while debugging is off, and a
    // setting that is read once at start instead of live.
    let mut runtime = build(SOURCE);
    let flag = Arc::new(AtomicBool::new(false));
    // The same control `enable_debug` attaches, with the switch that a config.set writes.
    let debug = DebugControl::new().with_enabled_flag(Arc::clone(&flag));
    runtime.set_debug_control(debug.clone());
    let (stop_tx, stop_rx) = channel();
    debug.set_stop_sender(stop_tx);
    let location = statement_location(&runtime, SOURCE, "n := n + DINT#10;");
    debug.set_breakpoints_for_file(location.file_id, vec![DebugBreakpoint::new(location)]);

    runtime.execute_cycle().expect("the first scan completes");
    runtime.execute_cycle().expect("the second scan completes");

    assert!(
        stop_rx.try_recv().is_err(),
        "a disabled debugger stops at no breakpoint"
    );
    assert_eq!(debug.storage_copies(), 0, "no storage is copied");
    assert!(debug.snapshot().is_none(), "no state is published");
    assert_eq!(
        runtime.storage().get_global("n"),
        Some(&Value::DInt(22)),
        "both scans run to completion as they would without a debugger"
    );

    // Enabling the setting while the runtime runs makes the next scan publish, with no restart.
    debug.set_breakpoints_for_file(location.file_id, Vec::new());
    flag.store(true, std::sync::atomic::Ordering::Relaxed);
    runtime.execute_cycle().expect("the third scan completes");
    assert_eq!(debug.storage_copies(), 1, "the enabled scan copies once");
    assert_eq!(published_dint(&debug, "n"), Some(Value::DInt(33)));
}

#[test]
fn debug_variables_between_scans_show_the_last_completed_scan() {
    // Spec 11 section 6.13.1, bullet 1: the published state is the storage at the end of the last
    // completed scan, and a scan in progress is not visible. Wrong behaviour caught: a read that shows
    // the partly updated storage of the scan in progress. Here the second scan has run both of its
    // statements (n = 22) but its output driver has not returned, so the scan has not completed.
    let (entered_tx, entered_rx) = channel();
    let (release_tx, release_rx) = channel();
    let mut runtime = build(SOURCE);
    runtime.add_io_driver(
        "gate",
        Box::new(GateDriver {
            entered: entered_tx,
            release: release_rx,
        }),
    );
    let debug = runtime.enable_debug();

    release_tx
        .send(())
        .expect("the first scan's output may return at once");
    runtime.execute_cycle().expect("the first scan completes");
    while entered_rx.try_recv().is_ok() {}
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(11)),
        "between scans, the state is the last completed scan's"
    );

    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });
    entered_rx
        .recv_timeout(WAIT)
        .expect("the second scan reaches its output driver");
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(11)),
        "a scan in progress is not visible: n is 22 in the live storage, 11 in the state"
    );

    release_tx.send(()).expect("the second scan may complete");
    let (runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the second scan completes");
    assert_eq!(runtime.storage().get_global("n"), Some(&Value::DInt(22)));
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(22)),
        "once the second scan completes, its end is the published state"
    );
}

#[test]
fn before_the_first_scan_completes_debug_reads_answer_no_snapshot_available() {
    // Spec 11 section 6.13.1, bullet 5: before the first completed scan there is no published state,
    // and each debug read answers its existing error, `no snapshot available`. Wrong behaviour caught:
    // an empty projection invented for a read, or a scan in progress published as the state. The
    // first scan is held in its output driver, after all of its statements have run.
    let (entered_tx, entered_rx) = channel();
    let (release_tx, release_rx) = channel();
    let mut runtime = build(SOURCE);
    runtime.add_io_driver(
        "gate",
        Box::new(GateDriver {
            entered: entered_tx,
            release: release_rx,
        }),
    );
    let debug = runtime.enable_debug();
    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });
    entered_rx
        .recv_timeout(WAIT)
        .expect("the first scan reaches its output driver");
    assert!(
        debug.snapshot().is_none(),
        "no scan has completed, so there is no state to read"
    );

    release_tx.send(()).expect("the first scan may complete");
    let (_runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the first scan completes");
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(11)),
        "the completed scan is the first published state"
    );
}

#[test]
fn debug_variables_during_a_hold_show_the_halted_state() {
    // Spec 11 section 6.13.1, bullet 4: while a debugger holds the scan, the state is the halted state,
    // the storage as the statements that already ran in the suspended scan left it. Wrong behaviour
    // caught: a read during the hold that shows the last completed scan (n = 11) instead of the stop
    // (n = 12: the first statement of the second scan has run, the second has not).
    let mut runtime = build(SOURCE);
    let debug = runtime.enable_debug();
    runtime.execute_cycle().expect("the first scan completes");

    let (stop_tx, stop_rx) = channel();
    debug.set_stop_sender(stop_tx);
    let location = statement_location(&runtime, SOURCE, "n := n + DINT#10;");
    debug.set_breakpoints_for_file(location.file_id, vec![DebugBreakpoint::new(location)]);
    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });

    let stop = stop_rx
        .recv_timeout(WAIT)
        .expect("the second scan stops at the breakpoint");
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);
    assert_eq!(stop.location, Some(location));
    assert!(debug.is_paused(), "the debugger holds the scan");
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(12)),
        "during the hold, the state is the halted state: n is 12, not the completed 11"
    );

    debug.continue_run();
    let (runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the second scan completes");
    assert_eq!(runtime.storage().get_global("n"), Some(&Value::DInt(22)));
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(22)),
        "after the hold ends, the state is the state of the completed scan"
    );
}

#[test]
fn a_debug_read_changes_no_value_hold_or_step() {
    // Spec 11 section 6.13.1, bullet 6: a read changes no value, breakpoint, hold or step. Wrong
    // behaviour caught: a read that releases the hold, consumes the step armed by the operator, or
    // applies a value, so that the step stops somewhere else or the scan ends with a different n.
    let mut runtime = build(SOURCE);
    let debug = runtime.enable_debug();
    runtime.execute_cycle().expect("the first scan completes");

    let (stop_tx, stop_rx) = channel();
    debug.set_stop_sender(stop_tx);
    let second = statement_location(&runtime, SOURCE, "n := n + DINT#10;");
    let third = statement_location(&runtime, SOURCE, "lamp := n > DINT#0;");
    debug.set_breakpoints_for_file(second.file_id, vec![DebugBreakpoint::new(second)]);
    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });

    let first_stop = stop_rx.recv_timeout(WAIT).expect("the breakpoint stops");
    assert_eq!(first_stop.reason, DebugStopReason::Breakpoint);
    assert_eq!(first_stop.location, Some(second));
    for _ in 0..2 {
        assert_eq!(published_dint(&debug, "n"), Some(Value::DInt(12)));
    }
    assert!(debug.is_paused(), "a read does not release the hold");
    assert_eq!(
        debug.last_stop().map(|stop| stop.reason),
        Some(DebugStopReason::Breakpoint),
        "a read does not change the last stop"
    );
    assert_eq!(
        debug.hold_reason(),
        Some(DebugStopReason::Breakpoint),
        "a read does not change why the scan is held"
    );

    debug.step_over();
    let step_stop = stop_rx
        .recv_timeout(WAIT)
        .expect("the step stops at the next statement");
    assert_eq!(
        step_stop.reason,
        DebugStopReason::Step,
        "the step armed before the reads still fires"
    );
    assert_eq!(step_stop.location, Some(third));
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(22)),
        "the second statement ran before the step stopped: 12 + 10"
    );

    debug.continue_run();
    let (runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the second scan completes");
    assert_eq!(
        runtime.storage().get_global("n"),
        Some(&Value::DInt(22)),
        "the reads and the step leave the final value unchanged"
    );
    assert_eq!(
        runtime.storage().get_global("lamp"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn after_a_faulted_scan_reads_show_the_values_the_fault_left() {
    // Spec 11 section 6.13.1 and owner decision O2 (18 October 2026): after a faulted scan the reads show
    // the storage as the fault left it, and the state names the position of the faulting statement. The
    // second scan stores n = 2 (1 + 1) before the division by zero, and the failing statement stores
    // nothing, so ratio keeps 10 from the first scan. The faulting statement is `ratio := ...;`, line 9
    // and column 1 of FAULT_SOURCE, counted from 1 with the empty first line.
    // Wrong behaviour caught: publishing the last completed scan (n = 1), which hides the store the
    // faulted scan made; a faulted state without a position; a failing statement that stores its
    // partial result (ratio = 0); a position on the scan's first statement (line 8).
    let mut runtime = build(FAULT_SOURCE);
    let debug = runtime.enable_debug();
    runtime.execute_cycle().expect("the first scan completes");

    let faulted = runtime.execute_cycle();
    assert!(
        matches!(faulted, Err(RuntimeError::DivisionByZero)),
        "the second scan divides by zero: {faulted:?}"
    );
    assert_eq!(
        runtime.storage().get_global("n"),
        Some(&Value::DInt(2)),
        "the faulted scan stored n before it failed"
    );
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(2)),
        "reads show the storage as the fault left it: n is 2, the store the scan completed"
    );
    assert_eq!(
        published_dint(&debug, "ratio"),
        Some(Value::DInt(10)),
        "the failing statement stores nothing: ratio keeps 10 from 10 / (2 - 1)"
    );
    let position = debug
        .fault_position()
        .expect("the faulted state names the faulting statement");
    assert_eq!(
        (position.line, position.column),
        (9, 1),
        "the faulting statement is line 9, column 1: ratio := DINT#10 / (DINT#2 - n);"
    );
}

#[test]
fn a_scan_completing_after_a_fault_replaces_the_faulted_state() {
    // Spec 11 section 6.13.1: the faulted state stays published until the next scan ends, and that end
    // replaces it with no position. A tooling reset of the fault keeps the values, so the scan after it
    // runs on n = 2: n = 2 + 1 = 3, and ratio = 10 / (2 - 3) = -10.
    // Wrong behaviour caught: the faulted state kept after a scan that completes (n = 2, or the position
    // still named); a reset that drops the faulted state before any scan has ended.
    let mut runtime = build(FAULT_SOURCE);
    let debug = runtime.enable_debug();
    runtime.execute_cycle().expect("the first scan completes");
    assert!(
        matches!(runtime.execute_cycle(), Err(RuntimeError::DivisionByZero)),
        "the second scan divides by zero"
    );

    runtime.clear_fault();
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(2)),
        "the reset keeps the values, and no scan has ended since the fault"
    );
    assert_eq!(
        debug.fault_position().map(|position| position.line),
        Some(9),
        "the faulted state still names line 9 before a scan ends"
    );

    runtime
        .execute_cycle()
        .expect("the scan after the reset completes");
    assert_eq!(published_dint(&debug, "n"), Some(Value::DInt(3)));
    assert_eq!(published_dint(&debug, "ratio"), Some(Value::DInt(-10)));
    assert_eq!(
        debug.fault_position(),
        None,
        "the completed scan replaces the faulted state, which names no position"
    );
    assert_eq!(
        debug.storage_copies(),
        3,
        "three scans ended, and each made one copy"
    );
}
