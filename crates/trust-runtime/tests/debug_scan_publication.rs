//! Debugger reads of a compiled program's scan (specification 11 section 6.13.1, section 6.9;
//! specification 13 "Variables / Evaluate"; specification 44 §13.2), through the control protocol's
//! debug operations and the state the debug control publishes.
//!
//! Expected values are worked out by hand from the programs below. `n` starts at 0 and the program
//! of `SOURCE` adds 1, then 10, then sets `lamp`, so the first scan ends with `n = 11` and the second
//! with `n = 22`. A stop before the second statement of the second scan holds `n = 12`.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value as Json};
use trust_runtime::debug::DebugControl;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::SourceFile;
use trust_runtime::io::IoDriver;
use trust_runtime::native::{build as build_image, Options, Target, Toolchain};
use trust_runtime::value::Value;
use trust_runtime::{CompiledDebugger, CompiledRuntime};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::ControlRequest;

/// The path the programs' source has, which the breakpoint requests name.
const PATH: &str = "src/main.st";

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

/// The program of `source` at [`PATH`], built for this computer with debug hooks and loaded.
fn build(source: &str) -> CompiledRuntime {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!(
        "trust-scan-publication-{}-{:?}",
        std::process::id(),
        thread::current().id()
    ));
    let built = build_image(
        &[SourceFile::with_path(PATH, source)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the fixture compiles");
    let _ = std::fs::remove_dir_all(&work);
    CompiledRuntime::load(built.image, "RESOURCE").expect("the image loads")
}

/// A debug control for `runtime`, and the debugger of its compiled program.
fn attach(runtime: &mut CompiledRuntime) -> (DebugControl, CompiledDebugger) {
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    let debugger = debug.compiled().expect("the compiled program's debugger");
    (debug, debugger)
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

/// The zero-based line of `source` that contains `needle`.
fn line_of(source: &str, needle: &str) -> u64 {
    source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing line for {needle}")) as u64
}

/// Sets the breakpoints of [`PATH`] on the zero-based `lines`.
fn set_breakpoints(debugger: &CompiledDebugger, lines: &[u64]) {
    let answer = ask(
        debugger,
        "breakpoints.set",
        Some(json!({ "source": PATH, "lines": lines })),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
}

/// The stops the debugger recorded since the last read.
fn stops(debugger: &CompiledDebugger) -> Vec<Json> {
    ask(debugger, "debug.stops", None)["result"]["stops"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// The next stop the debugger records within `WAIT`: its reason and zero-based line.
fn next_stop(debugger: &CompiledDebugger) -> (String, u64) {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(stop) = stops(debugger).first() {
            return (
                stop["reason"].as_str().expect("a reason").to_string(),
                stop["line"].as_u64().expect("a line"),
            );
        }
        assert!(Instant::now() < deadline, "no stop within the wait");
        thread::sleep(Duration::from_millis(2));
    }
}

/// The `debug.state` result: whether the scan is held and the last stop.
fn state(debugger: &CompiledDebugger) -> Json {
    ask(debugger, "debug.state", None)["result"].clone()
}

/// The value of a global in the state reads answer from, if a state is published.
fn published_dint(debug: &DebugControl, name: &str) -> Option<Value> {
    debug.snapshot().and_then(|snapshot| {
        snapshot
            .compiled
            .as_ref()
            .and_then(|state| state.value(name))
    })
}

#[test]
fn a_scan_with_debugging_disabled_publishes_no_state() {
    // Spec 11 section 6.9: debug hooks are side-effect-free when disabled, and a config.set of
    // runtime.control.debug_enabled takes effect as the setting already does. Wrong behaviour caught:
    // the hook publishing a state or holding the scan at a breakpoint while debugging is off, and a
    // setting that is read once at start instead of live.
    let mut runtime = build(SOURCE);
    let flag = Arc::new(AtomicBool::new(false));
    // A control with the switch that a config.set writes.
    let debug = DebugControl::new().with_enabled_flag(Arc::clone(&flag));
    runtime.set_debug_control(debug.clone());
    let debugger = debug.compiled().expect("the compiled program's debugger");
    set_breakpoints(&debugger, &[line_of(SOURCE, "n := n + DINT#10;")]);

    runtime.execute_cycle().expect("the first scan completes");
    runtime.execute_cycle().expect("the second scan completes");

    assert!(
        stops(&debugger).is_empty(),
        "a disabled debugger stops at no breakpoint"
    );
    assert!(debug.snapshot().is_none(), "no state is published");
    assert_eq!(
        runtime.read_variable("n"),
        Ok(Value::DInt(22)),
        "both scans run to completion as they would without a debugger"
    );

    // Enabling the setting while the runtime runs makes the next scan publish, with no restart.
    let cleared = ask(&debugger, "breakpoints.clear_all", None);
    assert_eq!(cleared["ok"], json!(true), "{cleared}");
    flag.store(true, std::sync::atomic::Ordering::Relaxed);
    runtime.execute_cycle().expect("the third scan completes");
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
    let (debug, _debugger) = attach(&mut runtime);

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
    assert_eq!(runtime.read_variable("n"), Ok(Value::DInt(22)));
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
    let (debug, debugger) = attach(&mut runtime);
    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });
    entered_rx
        .recv_timeout(WAIT)
        .expect("the first scan reaches its output driver");
    let read = ask(
        &debugger,
        "debug.variables",
        Some(json!({ "variables_reference": 1 })),
    );
    assert_eq!(
        read["ok"],
        json!(false),
        "a debug read before the first completed scan fails: {read}"
    );
    assert_eq!(read["error"], json!("no snapshot available"), "{read}");
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
    let (debug, debugger) = attach(&mut runtime);
    runtime.execute_cycle().expect("the first scan completes");

    let line = line_of(SOURCE, "n := n + DINT#10;");
    set_breakpoints(&debugger, &[line]);
    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });

    assert_eq!(
        next_stop(&debugger),
        ("breakpoint".to_string(), line),
        "the second scan stops at the breakpoint"
    );
    assert_eq!(
        state(&debugger)["paused"],
        json!(true),
        "the debugger holds the scan"
    );
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(12)),
        "during the hold, the state is the halted state: n is 12, not the completed 11"
    );

    debugger.resume();
    let (runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the second scan completes");
    assert_eq!(runtime.read_variable("n"), Ok(Value::DInt(22)));
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
    let (debug, debugger) = attach(&mut runtime);
    runtime.execute_cycle().expect("the first scan completes");

    let second = line_of(SOURCE, "n := n + DINT#10;");
    let third = line_of(SOURCE, "lamp := n > DINT#0;");
    set_breakpoints(&debugger, &[second]);
    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });

    assert_eq!(
        next_stop(&debugger),
        ("breakpoint".to_string(), second),
        "the breakpoint stops"
    );
    for _ in 0..2 {
        assert_eq!(published_dint(&debug, "n"), Some(Value::DInt(12)));
    }
    let held = state(&debugger);
    assert_eq!(
        held["paused"],
        json!(true),
        "a read does not release the hold"
    );
    assert_eq!(
        held["last_stop"]["reason"],
        json!("breakpoint"),
        "a read does not change the last stop, which is why the scan is held"
    );

    let step = ask(&debugger, "step_over", None);
    assert_eq!(step["ok"], json!(true), "{step}");
    assert_eq!(
        next_stop(&debugger),
        ("step".to_string(), third),
        "the step armed before the reads still fires, at the next statement"
    );
    assert_eq!(
        published_dint(&debug, "n"),
        Some(Value::DInt(22)),
        "the second statement ran before the step stopped: 12 + 10"
    );

    debugger.resume();
    let (runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the second scan completes");
    assert_eq!(
        runtime.read_variable("n"),
        Ok(Value::DInt(22)),
        "the reads and the step leave the final value unchanged"
    );
    assert_eq!(runtime.read_variable("lamp"), Ok(Value::Bool(true)));
}

#[test]
fn after_a_faulted_scan_reads_show_the_values_the_fault_left() {
    // Spec 11 section 6.13.1 and owner decision O2 (18 October 2026): after a faulted scan the reads show
    // the storage as the fault left it. The second scan stores n = 2 (1 + 1) before the division by
    // zero, and the failing statement stores nothing, so ratio keeps 10 from the first scan.
    // Wrong behaviour caught: publishing the last completed scan (n = 1), which hides the store the
    // faulted scan made; a failing statement that stores its partial result (ratio = 0).
    let mut runtime = build(FAULT_SOURCE);
    let (debug, _debugger) = attach(&mut runtime);
    runtime.execute_cycle().expect("the first scan completes");

    let faulted = runtime.execute_cycle();
    assert!(
        matches!(faulted, Err(RuntimeError::DivisionByZero)),
        "the second scan divides by zero: {faulted:?}"
    );
    assert_eq!(
        runtime.read_variable("n"),
        Ok(Value::DInt(2)),
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
}
