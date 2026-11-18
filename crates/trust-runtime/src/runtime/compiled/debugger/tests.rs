//! The computer's hold as a control thread reads it (specification 11 §6.13.1): the reads run on
//! the scan thread between the debugger's decision to hold the scan and the hold, the earliest a
//! control thread can find the hold's stop.
//!
//! `n` starts at 0 and the program adds 1, then 10, so the first scan ends with `n = 11`, a stop
//! before the second statement of the second scan holds `n = 12`, and the second scan ends with
//! `n = 22`. The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value as Json};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::ControlRequest;

use super::CompiledDebugger;
use crate::debug::DebugControl;
use crate::harness::SourceFile;
use crate::native::{build, Options, Target, Toolchain};
use crate::value::Value;
use crate::CompiledRuntime;

/// The path the program's source has, which the breakpoint request names.
const PATH: &str = "src/main.st";

const SOURCE: &str = r#"
VAR_GLOBAL
    n : DINT := DINT#0;
END_VAR

PROGRAM Main
n := n + DINT#1;
n := n + DINT#10;
END_PROGRAM
"#;

/// The longest the test waits for the scan to reach the breakpoint.
const WAIT: Duration = Duration::from_secs(30);

/// The program of `SOURCE`, built for this computer with debug hooks and loaded.
fn runtime() -> CompiledRuntime {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = trust_test_support::unique_temp_path(&format!(
        "trust-hold-state-{}-{:?}",
        std::process::id(),
        thread::current().id()
    ));
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
    CompiledRuntime::load(built.image, "RESOURCE").expect("the image loads")
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

/// The zero-based line of `SOURCE` that contains `needle`.
fn line_of(needle: &str) -> u64 {
    SOURCE
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing line for {needle}")) as u64
}

#[test]
fn a_read_that_finds_a_holds_stop_answers_from_the_halted_state() {
    // Spec 11 §6.13.1: while a debugger holds the scan, the published state is the halted state.
    // Wrong behaviour caught: the stop recorded, and so read, before the held state is published,
    // so that a control thread that reads the values as soon as it finds the stop gets the last
    // scan's `n = 11` at a breakpoint where `n` is 12 (#599's Windows CI, 18 November 2026).
    let mut runtime = runtime();
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    let debugger = debug.compiled().expect("the compiled program's debugger");
    runtime.execute_cycle().expect("the first scan completes");

    let line = line_of("n := n + DINT#10;");
    let set = ask(
        &debugger,
        "breakpoints.set",
        Some(json!({ "source": PATH, "lines": [line] })),
    );
    assert_eq!(set["ok"], json!(true), "{set}");
    let (send, reads) = channel();
    *debugger
        .shared
        .hold_decided
        .lock()
        .expect("no test thread panicked") = Some(Box::new(move |debugger: &CompiledDebugger| {
        let stops = ask(debugger, "debug.stops", None)["result"]["stops"].clone();
        let n = debugger.state().and_then(|state| state.value("n"));
        let _ = send.send((stops, n));
    }));
    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });

    let (stops, n) = reads
        .recv_timeout(WAIT)
        .expect("the second scan holds at the breakpoint");
    let stops = stops.as_array().cloned().unwrap_or_default();
    assert_eq!(stops.len(), 1, "the read finds the hold's stop: {stops:?}");
    assert_eq!(stops[0]["reason"], json!("breakpoint"));
    assert_eq!(stops[0]["line"], json!(line));
    assert_eq!(
        n,
        Some(Value::DInt(12)),
        "the read that finds the stop answers from the halted state, not from the last scan's 11"
    );

    debugger.resume();
    let (runtime, result) = scan.join().expect("the scan thread ends");
    result.expect("the second scan completes");
    assert_eq!(runtime.read_variable("n"), Ok(Value::DInt(22)));
    assert_eq!(
        debugger.state().and_then(|state| state.value("n")),
        Some(Value::DInt(22)),
        "after the hold, the reads answer from the scan that ended"
    );
}
