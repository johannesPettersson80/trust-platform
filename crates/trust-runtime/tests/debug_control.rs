//! The debugger of a program compiled for this computer, through the control protocol's debug
//! operations (specification 13; specification 11 §6.13; specification 44 §13.2, §8.8): a
//! breakpoint holds the scan at its statement and reports the stop, a step holds again at the
//! statement its kind selects, a condition, a hit count and a logpoint decide each hit, a hold keeps
//! the order of the tasks, and a breakpoint request resolves a line to the statement on it or after
//! it. The debug control keeps the runtime events that no receiver took.
//!
//! Every stop is read from `debug.stops`, which records it with the decision to hold, so no read
//! races the copy of the held scan's memory.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;
use std::thread::JoinHandle;
use std::time::{Duration as WallDuration, Instant};

use base64::Engine as _;
use serde_json::{json, Value as Json};
use trust_hir::types::TypeRegistry;
use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{compile_debug_expression, SourceFile};
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::value::{DateTimeProfile, Duration, Value};
use trust_runtime::{CompiledDebugger, CompiledRuntime, RestartMode};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::ControlRequest;

/// The path the program's source has, which the breakpoint requests name.
const PATH: &str = "src/main.st";

/// Zero-based lines: 4 `AddTwo := x + 2;`, 12 `count := count + 1;`, 13 blank, 14 `total :=
/// AddTwo(count);`, 15 `IF count > 0 THEN`, 16 `total := total + 1;` (column 4). The 10 ms task
/// runs `P` in the scans at 10, 20 and 30 ms (specification 10 §4.3); the scan at 10 ms makes
/// `count` 1 and `total` 1 + 2 + 1 = 4, the one at 20 ms 2 and 5, the one at 30 ms 3 and 6.
const SOURCE: &str = "FUNCTION AddTwo : DINT
VAR_INPUT
    x : DINT;
END_VAR
AddTwo := x + 2;
END_FUNCTION

PROGRAM Main
VAR
    count : DINT;
    total : DINT;
END_VAR
count := count + 1;

total := AddTwo(count);
IF count > 0 THEN
    total := total + 1;
END_IF;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// Two tasks due in the same scan: `Fast` (priority 1) runs `P1`, which appends 1 to `trace`, and
/// then `Slow` (priority 2) runs `P2`, which appends 2. Zero-based line 4 is `P1`'s statement.
const TWO_TASKS: &str = "PROGRAM Prog1
VAR_EXTERNAL
    trace : INT;
END_VAR
trace := trace * INT#10 + INT#1;
END_PROGRAM

PROGRAM Prog2
VAR_EXTERNAL
    trace : INT;
END_VAR
trace := trace * INT#10 + INT#2;
END_PROGRAM

CONFIGURATION Conf
VAR_GLOBAL
    trace : INT := 0;
END_VAR
RESOURCE Cpu ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#10ms, PRIORITY := 2);
    PROGRAM P1 WITH Fast : Prog1;
    PROGRAM P2 WITH Slow : Prog2;
END_RESOURCE
END_CONFIGURATION
";

/// The debug image of `source` at [`PATH`] for this computer.
fn image(source: &str, name: &str) -> Vec<u8> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!("trust-debug-{name}-{}", std::process::id()));
    let built = build(
        &[SourceFile::with_path(PATH, source)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    built.image
}

/// The image of [`SOURCE`], built once for the tests that share it.
fn source_image() -> Vec<u8> {
    static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
    IMAGE.get_or_init(|| image(SOURCE, "source")).clone()
}

/// The program of `image` with its debugger, cold started with its tasks' schedule at 0 ms.
fn debugged(image: Vec<u8>) -> (CompiledRuntime, DebugControl, CompiledDebugger) {
    let mut runtime = CompiledRuntime::load(image, "Cpu").expect("the image loads");
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    runtime.restart(RestartMode::Cold).expect("the cold start");
    runtime.reset_task_timing(Duration::ZERO);
    let debugger = debug.compiled().expect("the compiled program's debugger");
    (runtime, debug, debugger)
}

/// A scan on its own thread, as the resource's scan thread runs it; `done` receives the scan's
/// result when it ends.
struct Scan {
    thread: JoinHandle<CompiledRuntime>,
    done: Receiver<Result<(), RuntimeError>>,
}

impl Scan {
    /// Runs the scan at `ms` milliseconds.
    fn at(mut runtime: CompiledRuntime, ms: i64) -> Self {
        let (sender, done) = channel();
        let thread = std::thread::spawn(move || {
            runtime.set_current_time(Duration::from_millis(ms));
            let _ = sender.send(runtime.execute_cycle());
            runtime
        });
        Self { thread, done }
    }

    /// Whether the scan is still running, after 100 ms more.
    fn is_held(&self) -> bool {
        self.done
            .recv_timeout(WallDuration::from_millis(100))
            .is_err()
    }

    /// The runtime, once the scan ended without an error.
    fn ends(self) -> CompiledRuntime {
        let result = self
            .done
            .recv_timeout(WallDuration::from_secs(10))
            .expect("the scan ends within 10 s");
        result.expect("the scan completes");
        self.thread.join().expect("the scan thread ends")
    }
}

/// A scan that no breakpoint holds: the runtime after it.
fn scan(runtime: CompiledRuntime, ms: i64) -> CompiledRuntime {
    Scan::at(runtime, ms).ends()
}

fn request(id: u64, kind: &str, params: Option<Json>) -> ControlRequest {
    ControlRequest {
        id,
        r#type: kind.to_string(),
        params,
        auth: None,
        request_id: None,
    }
}

/// The JSON of the debugger's answer to `kind` with `params`.
fn ask(debugger: &CompiledDebugger, kind: &str, params: Option<Json>) -> Json {
    let response = debugger.answer(&request(1, kind, params), IoSnapshotRead::Missing);
    serde_json::to_value(&response).expect("a response serializes")
}

/// Sets the breakpoints of [`PATH`] on the zero-based `lines`, with `options` when given, and gives
/// the answer's result.
fn set_breakpoints(debugger: &CompiledDebugger, lines: &[u32], options: Option<Json>) -> Json {
    let mut params = json!({ "source": PATH, "lines": lines });
    if let Some(options) = options {
        params["options"] = options;
    }
    let answer = ask(debugger, "breakpoints.set", Some(params));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    answer["result"].clone()
}

/// The stops and the logpoint messages since the last read: `(stops, logs)`.
fn drain(debugger: &CompiledDebugger) -> (Vec<Json>, Vec<Json>) {
    let answer = ask(debugger, "debug.stops", None);
    let list = |key: &str| {
        answer["result"][key]
            .as_array()
            .cloned()
            .unwrap_or_default()
    };
    (list("stops"), list("logs"))
}

/// The next stop, once the scan arrives at it: its reason and zero-based line, after checking that
/// it is the only one and stands in [`PATH`].
fn next_stop(debugger: &CompiledDebugger) -> (String, u64) {
    let deadline = Instant::now() + WallDuration::from_secs(10);
    loop {
        let (stops, _) = drain(debugger);
        if let [stop] = stops.as_slice() {
            assert_eq!(stop["path"], json!(PATH), "{stop}");
            let reason = stop["reason"].as_str().expect("a reason").to_string();
            return (reason, stop["line"].as_u64().expect("a line"));
        }
        assert!(stops.is_empty(), "one stop at a time: {stops:?}");
        assert!(Instant::now() < deadline, "no stop within 10 s");
        std::thread::sleep(WallDuration::from_millis(5));
    }
}

/// The value of `name` that the last scan left.
fn value(runtime: &CompiledRuntime, name: &str) -> Option<Value> {
    runtime
        .snapshot()
        .expect("the program keeps its image")
        .value(name)
}

/// The compiled form of the debug expression `text` as the protocol carries it (specification 11
/// §6.13.2).
fn expression(text: &str) -> Json {
    let bytes = compile_debug_expression(
        text,
        &mut TypeRegistry::new(),
        DateTimeProfile::default(),
        &[],
    )
    .expect("the expression compiles");
    json!({
        "expr_version": 1,
        "expr": base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

/// Specification 13, "Runtime Debug-Control Execution": a breakpoint on line 12 holds the scan at
/// 10 ms before `count := count + 1;` and reports one stop of reason `breakpoint` there, with the
/// generation the request answered; the scan stays held until `resume`, and then ends with `count`
/// 1 and `total` 4. Catches a breakpoint that does not hold, a stop without its place or
/// generation, a hold that ends without a resume, and a resume that does not run the rest of the
/// scan.
#[test]
fn a_breakpoint_holds_the_scan_until_resume() {
    let (runtime, _debug, debugger) = debugged(source_image());
    let set = set_breakpoints(&debugger, &[12], None);

    let held = Scan::at(runtime, 10);
    assert_eq!(next_stop(&debugger), ("breakpoint".to_string(), 12));
    assert!(held.is_held());
    let state = ask(&debugger, "debug.state", None);
    assert_eq!(state["result"]["paused"], json!(true), "{state}");
    assert_eq!(
        state["result"]["last_stop"]["breakpoint_generation"], set["generation"],
        "{state}"
    );

    debugger.resume();
    let runtime = held.ends();
    assert_eq!(value(&runtime, "P.count"), Some(Value::DInt(1)));
    assert_eq!(value(&runtime, "P.total"), Some(Value::DInt(4)));
}

/// Specification 13, "Step Over": held at line 12, a step over holds again at the next statement
/// of the same frame, line 14, with reason `step`; held at line 14, which calls `AddTwo`, a step
/// over runs the call and holds at line 15, not in `AddTwo` at line 4. Catches a step that runs on
/// to the end of the scan, and a step over that stops in the called function.
#[test]
fn step_over_holds_at_the_next_statement_of_the_frame() {
    let (runtime, _debug, debugger) = debugged(source_image());
    set_breakpoints(&debugger, &[12], None);

    let held = Scan::at(runtime, 10);
    assert_eq!(next_stop(&debugger), ("breakpoint".to_string(), 12));
    let step = ask(&debugger, "step_over", None);
    assert_eq!(step["ok"], json!(true), "{step}");
    assert_eq!(next_stop(&debugger), ("step".to_string(), 14));
    assert!(held.is_held());
    ask(&debugger, "step_over", None);
    assert_eq!(next_stop(&debugger), ("step".to_string(), 15));
    assert!(held.is_held());

    debugger.resume();
    let runtime = held.ends();
    assert_eq!(value(&runtime, "P.total"), Some(Value::DInt(4)));
}

/// Specification 13, "Step In": held at line 14, `total := AddTwo(count);`, a step in holds at the
/// first statement of the called function, line 4. Catches a step in that runs the call without
/// stopping in it.
#[test]
fn step_in_holds_in_the_called_function() {
    let (runtime, _debug, debugger) = debugged(source_image());
    set_breakpoints(&debugger, &[14], None);

    let held = Scan::at(runtime, 10);
    assert_eq!(next_stop(&debugger), ("breakpoint".to_string(), 14));
    ask(&debugger, "step_in", None);
    assert_eq!(next_stop(&debugger), ("step".to_string(), 4));
    assert!(held.is_held());

    debugger.resume();
    held.ends();
}

/// Specification 13, "Step Out": held in `AddTwo` at line 4, a step out runs the rest of the
/// function and holds at the caller's next statement, line 15. Catches a step out that stops in
/// the function again or runs on to the end of the scan.
#[test]
fn step_out_holds_in_the_caller_after_the_return() {
    let (runtime, _debug, debugger) = debugged(source_image());
    set_breakpoints(&debugger, &[4], None);

    let held = Scan::at(runtime, 10);
    assert_eq!(next_stop(&debugger), ("breakpoint".to_string(), 4));
    ask(&debugger, "step_out", None);
    assert_eq!(next_stop(&debugger), ("step".to_string(), 15));
    assert!(held.is_held());

    debugger.resume();
    let runtime = held.ends();
    assert_eq!(value(&runtime, "P.total"), Some(Value::DInt(4)));
}

/// Specification 13, "Breakpoint evaluation, source resolution, and hook contract": a request on
/// the blank line 13 resolves to the earliest statement after it, line 14 column 0; a request on
/// line 15, where the `IF` starts, to the `IF` statement at column 0; a request on line 16, inside
/// the `IF`, to the inner statement, at the column of its first token, 4. Catches a resolution that
/// drops a line without a statement, and one that gives the enclosing `IF` for its inner statement.
#[test]
fn a_breakpoint_line_resolves_to_its_statement() {
    let (_runtime, _debug, debugger) = debugged(source_image());
    let set = set_breakpoints(&debugger, &[13, 15, 16], None);
    assert_eq!(
        set["resolved"],
        json!([
            { "line": 14, "column": 0 },
            { "line": 15, "column": 0 },
            { "line": 16, "column": 4 },
        ])
    );
}

/// Specification 13 and specification 11 §6.13.3: every replacement of a file's breakpoints, an
/// empty one too, answers a greater generation. Catches a clear that keeps the generation, which
/// would let a stop of the cleared breakpoint pass as current.
#[test]
fn breakpoint_generation_increments_on_clear() {
    let (_runtime, _debug, debugger) = debugged(source_image());
    let before = set_breakpoints(&debugger, &[12], None)["generation"]
        .as_u64()
        .expect("a generation");
    let after = set_breakpoints(&debugger, &[], None)["generation"]
        .as_u64()
        .expect("a generation");
    assert!(after > before, "{before} then {after}");
}

/// Specification 13, conditional breakpoints: the condition `count = DINT#2` on line 14 is FALSE
/// in the scan at 10 ms, where `count` is 1, so that scan ends without a stop; it is TRUE in the
/// scan at 20 ms, which holds there. Catches a condition that is not evaluated, and one evaluated
/// with other names than the frame's.
#[test]
fn a_condition_decides_each_hit() {
    let (runtime, _debug, debugger) = debugged(source_image());
    set_breakpoints(
        &debugger,
        &[14],
        Some(json!([{ "condition": expression("count = DINT#2") }])),
    );

    let runtime = scan(runtime, 10);
    assert_eq!(drain(&debugger), (Vec::new(), Vec::new()));
    let held = Scan::at(runtime, 20);
    assert_eq!(next_stop(&debugger), ("breakpoint".to_string(), 14));
    assert!(held.is_held());

    debugger.resume();
    let runtime = held.ends();
    assert_eq!(value(&runtime, "P.count"), Some(Value::DInt(2)));
}

/// Specification 13, hit conditions: the hit condition `= 3` on line 12 lets the hits in the scans
/// at 10 and 20 ms run and holds at the third, in the scan at 30 ms. Catches hits that are not
/// counted across scans, and a hit condition that holds at every hit.
#[test]
fn a_hit_count_holds_on_its_hit() {
    let (runtime, _debug, debugger) = debugged(source_image());
    set_breakpoints(
        &debugger,
        &[12],
        Some(json!([{ "hit_condition": { "op": "=", "count": 3 } }])),
    );

    let runtime = scan(scan(runtime, 10), 20);
    assert_eq!(drain(&debugger), (Vec::new(), Vec::new()));
    let held = Scan::at(runtime, 30);
    assert_eq!(next_stop(&debugger), ("breakpoint".to_string(), 12));
    assert!(held.is_held());

    debugger.resume();
    let runtime = held.ends();
    assert_eq!(value(&runtime, "P.count"), Some(Value::DInt(3)));
}

/// Specification 13, logpoints, with the logpoint text of specification 11 §6.13.2: the message
/// `count=` and `count` on line 14 logs `count=DInt(1)` in the scan at 10 ms, at line 14 of
/// [`PATH`], and the scan runs on without a stop. Catches a logpoint that holds, one that logs
/// nothing, and a message that evaluates its expression elsewhere than in the frame.
#[test]
fn a_logpoint_logs_without_holding() {
    let (runtime, _debug, debugger) = debugged(source_image());
    set_breakpoints(
        &debugger,
        &[14],
        Some(json!([{ "log_message": ["count=", expression("count")] }])),
    );

    let runtime = scan(runtime, 10);
    let (stops, logs) = drain(&debugger);
    assert!(stops.is_empty(), "{stops:?}");
    let [log] = logs.as_slice() else {
        panic!("one message: {logs:?}");
    };
    assert_eq!(log["text"], json!("count=DInt(1)"), "{log}");
    assert_eq!(log["line"], json!(14), "{log}");
    assert_eq!(log["path"], json!(PATH), "{log}");
    assert_eq!(value(&runtime, "P.count"), Some(Value::DInt(1)));
}

/// Specification 13, "Hold duration", with the task order of specification 10 §4.3: a breakpoint
/// in `P1` holds the scan at 10 ms inside the task `Fast`; after `resume` the scan runs `Slow`, so
/// `trace` is 1 then 12. Catches a hold that skips or repeats a task, or runs the tasks in another
/// order.
#[test]
fn a_hold_keeps_the_task_order() {
    let (runtime, _debug, debugger) = debugged(image(TWO_TASKS, "two-tasks"));
    set_breakpoints(&debugger, &[4], None);

    let held = Scan::at(runtime, 10);
    assert_eq!(next_stop(&debugger), ("breakpoint".to_string(), 4));
    assert!(held.is_held());

    debugger.resume();
    let runtime = held.ends();
    assert_eq!(value(&runtime, "trace"), Some(Value::Int(12)));
}

/// Specification 11, the debug event buffer: a runtime event pushed after its receiver was dropped
/// stays in the debug control for a direct drain. Catches an event lost with its receiver.
#[test]
fn runtime_event_sender_drop_buffers_event_in_debug_control() {
    let control = DebugControl::new();
    let (tx, rx) = channel();
    drop(rx);
    control.set_runtime_sender(tx);

    control.push_runtime_event(RuntimeEvent::fault(
        &RuntimeError::SimulationFault("first fault".into()),
        Duration::ZERO,
    ));

    let events = control.drain_runtime_events();
    assert!(
        events.iter().any(|event| matches!(
            event,
            RuntimeEvent::Fault { fault, message, .. }
                if fault.render(message.as_ref()) == "simulation fault 'first fault'"
        )),
        "dropped runtime event sender must not lose the first fault, got {events:?}"
    );
}
