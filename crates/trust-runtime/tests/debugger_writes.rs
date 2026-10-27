#![cfg(feature = "full")]
//! A debugger's writes of a compiled program's variables on a computer (specification 44 §13.2 and
//! §13.3; specification 11 §6.13.4 and "Live Values"; specification 13, "setVariable and
//! setExpression"). At a hold, `debug.set` hands its write to the scan thread, which writes it into
//! the held scan at once, so the held statement and the reads at the hold see it; between scans it
//! writes nothing and answers the variable's path, by which the client queues the write with `set`
//! or forces it with `var.force` for the next cycle start, as the debug adapter does.
//!
//! The scan runs on a second thread, as the resource runs it, and the test thread answers the debug
//! operations meanwhile, as the control threads do (the threads of `debugger_hold_stop.rs`). Each
//! test releases the hold and joins the scan thread before it asserts, so a failed assertion never
//! leaves a thread waiting. The writes the client queues and the forces it sets go into
//! `DebugControl`'s queues, which the control requests `set`, `var.force` and `var.unforce` fill.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::sync::OnceLock;
use std::thread;
use std::time::{Duration as WallDuration, Instant};

use serde_json::{json, Value as Json};
use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::value::Value;
use trust_runtime::{CompiledDebugger, CompiledRuntime, RestartMode};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::ControlRequest;

/// The longest a test waits for a stop, so a missing hold fails instead of blocking.
const WAIT: WallDuration = WallDuration::from_secs(5);

/// `Main` runs four statements in each scan. The function `Scaled` multiplies its input by its
/// local `factor`, which each call starts at 2. In the first scan, without a write, `Second` is
/// 1 + 1 = 2, `Counts[2]` and `Seen` are 1, and `Third` is 1 * 2 = 2. An element of `Counts` has
/// no symbol of its own (specification 44 §5.7), so its path is found through the type table.
const SOURCE: &str = r#"
FUNCTION Scaled : DINT
VAR_INPUT
    x : DINT;
END_VAR
VAR
    factor : DINT := DINT#2;
END_VAR
Scaled := x * factor;
END_FUNCTION

PROGRAM Main
VAR
    First : DINT := DINT#1;
    Second : DINT := DINT#0;
    Third : DINT := DINT#0;
    Counts : ARRAY[1..2] OF DINT;
    Seen : DINT := DINT#0;
END_VAR
Second := First + DINT#1;
Counts[2] := Counts[2] + DINT#1;
Seen := Counts[2];
Third := Scaled(First);
END_PROGRAM
"#;

/// The path the program's source has, which the breakpoint requests name.
const PATH: &str = "src/main.st";

/// The image of [`SOURCE`] at [`PATH`], built once for this computer with debug hooks.
fn image() -> Vec<u8> {
    static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
    IMAGE
        .get_or_init(|| {
            let toolchain = Toolchain::find()
                .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
            let work =
                std::env::temp_dir().join(format!("trust-debugger-writes-{}", std::process::id()));
            let built = build(
                &[SourceFile::with_path(PATH, SOURCE)],
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
        })
        .clone()
}

/// The program loaded with a debug control, started cold as trust-runtime starts a program it
/// loads, and the debugger of its compiled program.
fn start() -> (CompiledRuntime, DebugControl, CompiledDebugger) {
    let mut runtime = CompiledRuntime::load(image(), "RESOURCE").expect("the image loads");
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    runtime.restart(RestartMode::Cold).expect("the cold start");
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

/// Sets the breakpoints of [`PATH`] on the zero-based source lines that contain `needles`.
fn set_breakpoints(debugger: &CompiledDebugger, needles: &[&str]) {
    let lines: Vec<usize> = needles
        .iter()
        .map(|needle| {
            SOURCE
                .lines()
                .position(|line| line.contains(needle))
                .unwrap_or_else(|| panic!("the source has no line containing {needle}"))
        })
        .collect();
    let answer = ask(
        debugger,
        "breakpoints.set",
        Some(json!({ "source": PATH, "lines": lines })),
    );
    assert_eq!(answer["ok"], json!(true), "{answer}");
}

/// The reason of the next stop the debugger records within [`WAIT`]; `None` when none arrives.
fn next_stop(debugger: &CompiledDebugger) -> Option<String> {
    let deadline = Instant::now() + WAIT;
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
        thread::sleep(WallDuration::from_millis(2));
    }
}

/// Runs one scan of `runtime` on a thread of its own, as the resource thread runs it.
fn scan_on_thread(
    mut runtime: CompiledRuntime,
) -> thread::JoinHandle<(CompiledRuntime, Result<(), RuntimeError>)> {
    thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    })
}

/// The value of the variable `name` as the last scan left it.
fn read(runtime: &CompiledRuntime, name: &str) -> Value {
    runtime
        .read_variable(name)
        .unwrap_or_else(|error| panic!("{name} reads: {error}"))
}

#[test]
fn a_write_at_a_breakpoint_goes_into_the_held_scan_and_the_held_statement_sees_it() {
    // Specification 44 §13.2 and specification 11 §6.13.4, worked by hand: the first scan holds at
    // `Second := First + DINT#1;` with `First` 1. `debug.set` of `First`, the one program's
    // variable of that name, := 41 is written into the held scan at once: the answer gives the
    // variable's path, `Main.First` (its symbol's display name, §5.7), DINT and the value the held
    // scan holds after the write, 41; a read at the hold shows 41. Released, the held statement
    // runs with 41: `Second` = 42, and `Third` = Scaled(41) = 41 * 2 = 82.
    // Catches a write that `debug.set` only planned (Second would be 2), a write into the copy the
    // reads answer from but not into the held scan (the read shows 41, Second 2), and an answer
    // built before the scan thread made the write (it would report 1).
    let (runtime, _debug, debugger) = start();
    set_breakpoints(&debugger, &["Second := First + DINT#1;"]);
    let scan = scan_on_thread(runtime);
    let stop = next_stop(&debugger);
    let written = ask(
        &debugger,
        "debug.set",
        Some(json!({"path": "First", "value": "41"})),
    );
    let at_hold = ask(&debugger, "debug.set", Some(json!({"path": "Main.First"})));
    debugger.resume();
    let (runtime, result) = scan.join().expect("the scan thread ends");

    assert_eq!(stop.as_deref(), Some("breakpoint"), "the scan holds");
    assert_eq!(
        written["result"],
        json!({"path": "Main.First", "type": "DINT", "value": "41", "written": true}),
        "{written}"
    );
    assert_eq!(
        at_hold["result"],
        json!({"path": "Main.First", "type": "DINT", "value": "41", "written": false}),
        "{at_hold}"
    );
    assert_eq!(result, Ok(()), "the released scan ends");
    assert_eq!(read(&runtime, "Main.Second"), Value::DInt(42));
    assert_eq!(read(&runtime, "Main.Third"), Value::DInt(82));
}

#[test]
fn a_write_to_a_local_of_a_function_frame_at_a_hold_reaches_the_function() {
    // Specification 44 §13.2, worked by hand: held in `Scaled`, which `Main` calls with `x` =
    // `First` = 1, the innermost frame of the stack is the function's, and its local `factor`, 2
    // by its declaration, is written 3 into that frame at once. A frame variable has no path
    // (§5.7: area 7, for the debugger only), so the answer's path is null, and a force of it is
    // refused, as no force could name it once the frame is gone. Released, the function returns
    // 1 * 3 = 3 to `Third`.
    // Catches a frame variable written into another frame or an area (Third would be 2), and a
    // force of a local accepted that would hold nothing.
    let (runtime, _debug, debugger) = start();
    set_breakpoints(&debugger, &["Scaled := x * factor;"]);
    let scan = scan_on_thread(runtime);
    let stop = next_stop(&debugger);
    let stack = ask(&debugger, "debug.stack", None);
    let innermost = stack["result"]["stack_frames"][0].clone();
    let frame_id = innermost["id"].clone();
    let written = ask(
        &debugger,
        "debug.set",
        Some(json!({"path": "factor", "frame_id": frame_id, "value": "3"})),
    );
    let forced = ask(
        &debugger,
        "debug.set",
        Some(json!({"path": "factor", "frame_id": frame_id, "value": "4", "force": true})),
    );
    debugger.resume();
    let (runtime, result) = scan.join().expect("the scan thread ends");

    assert_eq!(stop.as_deref(), Some("breakpoint"), "the scan holds");
    assert_eq!(innermost["name"], json!("Scaled"), "{stack}");
    assert_eq!(
        written["result"],
        json!({"path": null, "type": "DINT", "value": "3", "written": true}),
        "{written}"
    );
    assert_eq!(
        forced["error"],
        json!("a variable of a frame on the held call stack cannot be forced: write it instead"),
        "{forced}"
    );
    assert_eq!(result, Ok(()), "the released scan ends");
    assert_eq!(read(&runtime, "Main.Third"), Value::DInt(3));
}

#[test]
fn a_write_between_scans_is_queued_by_its_path_and_reaches_the_next_scan() {
    // Specification 11 §6.13.4 and "Live Values", specification 44 §13.3, worked by hand: with no
    // scan held, `debug.set` of `main.FIRST` (names compare without case) := 5 writes nothing and
    // answers the variable's path, `Main.First`, with the admitted value; the client queues it with
    // `set` by that path, and the next scan's cycle start applies it: `Second` = 6, `Third` = 10.
    // Catches a write between scans that bypassed the cycle start (First would read 5 before the
    // scan), and a path the cycle start cannot resolve (refused as `write_rejected`, Second 2).
    let (mut runtime, debug, debugger) = start();
    runtime.execute_cycle().expect("the first scan");
    let answer = ask(
        &debugger,
        "debug.set",
        Some(json!({"path": "main.FIRST", "value": "5"})),
    );
    let before = read(&runtime, "Main.First");
    let path = answer["result"]["path"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    debug
        .try_enqueue_global_write(path, Value::DInt(5))
        .expect("a new target is inside the bound");
    runtime.execute_cycle().expect("the second scan");
    let rejected: Vec<RuntimeEvent> = debug
        .drain_runtime_events()
        .into_iter()
        .filter(|event| matches!(event, RuntimeEvent::WriteRejected { .. }))
        .collect();

    assert_eq!(
        answer["result"],
        json!({"path": "Main.First", "type": "DINT", "value": "5", "written": false}),
        "{answer}"
    );
    assert_eq!(before, Value::DInt(1), "nothing is written between scans");
    assert!(rejected.is_empty(), "{rejected:?}");
    assert_eq!(read(&runtime, "Main.Second"), Value::DInt(6));
    assert_eq!(read(&runtime, "Main.Third"), Value::DInt(10));
}

#[test]
fn a_force_of_an_element_at_a_hold_is_written_at_once_and_holds_until_it_is_released() {
    // Specification 44 §13.2 and §13.3, specification 11 "Live Values", worked by hand: the first
    // scan holds at `Counts[2] := Counts[2] + DINT#1;` with `Counts[2]` 0. `debug.set` of
    // `Counts[2]`, the element of the one program's `Counts`, := 100 with `force` is written into
    // the held scan at once (the answer reads 100 back from it) and names its path,
    // `Main.Counts[2]`, by which the client sets the force (`var.force`). Released, the scan counts
    // 100 to 101 and `Seen` takes 101; the force sets the element to 100 again before the outputs.
    // The next scan starts from the force, 100, counts to 101 (Seen 101) and ends at 100 again.
    // After the release (`var.unforce` by the path a read gives), the next scan starts from 100 and
    // keeps its count, 101; the one after counts to 102.
    // Catches a force at a hold that is set but not written into the held scan (the answer would
    // not be written), a forced path that the scan's boundaries cannot resolve (the element would
    // count on: 101, 102), a force that holds only at the cycle start (101 after a scan), and a
    // release that leaves the force (100 after it).
    let (runtime, debug, debugger) = start();
    set_breakpoints(&debugger, &["Counts[2] := Counts[2] + DINT#1;"]);
    let scan = scan_on_thread(runtime);
    let stop = next_stop(&debugger);
    let forced = ask(
        &debugger,
        "debug.set",
        Some(json!({"path": "Counts[2]", "value": "100", "force": true})),
    );
    let path = forced["result"]["path"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    debug
        .try_force_global(path.as_str(), Value::DInt(100))
        .expect("a new target is inside the bound");
    let cleared = ask(&debugger, "breakpoints.clear_all", None);
    debugger.resume();
    let (mut runtime, result) = scan.join().expect("the scan thread ends");
    let counted =
        |runtime: &CompiledRuntime| (read(runtime, "Main.Counts[2]"), read(runtime, "Main.Seen"));
    let mut scans = vec![counted(&runtime)];
    runtime.execute_cycle().expect("the second scan");
    scans.push(counted(&runtime));
    let release = ask(&debugger, "debug.set", Some(json!({"path": "Counts[2]"})));
    debug.release_global(release["result"]["path"].as_str().unwrap_or_default());
    for _ in 0..2 {
        runtime.execute_cycle().expect("a scan after the release");
        scans.push(counted(&runtime));
    }

    assert_eq!(stop.as_deref(), Some("breakpoint"), "the scan holds");
    assert_eq!(
        forced["result"],
        json!({"path": "Main.Counts[2]", "type": "DINT", "value": "100", "written": true}),
        "{forced}"
    );
    assert_eq!(cleared["ok"], json!(true), "{cleared}");
    assert_eq!(result, Ok(()), "the released scan ends");
    assert_eq!(
        release["result"],
        json!({"path": "Main.Counts[2]", "type": "DINT", "value": "100", "written": false}),
        "{release}"
    );
    let dint = |count, seen| (Value::DInt(count), Value::DInt(seen));
    assert_eq!(
        scans,
        [
            dint(100, 101),
            dint(100, 101),
            dint(101, 101),
            dint(102, 102)
        ]
    );
}
