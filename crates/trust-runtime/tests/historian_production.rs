//! The historian of a running `trust-runtime` in production (specification 11, "Runtime historian
//! recording and export contract"; specification 44 NX-14): the real `trust-runtime` runs a private
//! copy of the example `memory_marker_counter` with debugging off, as the example and `trust-runtime
//! new` set it, and its observability on, and the history file receives the program's values.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::time::Duration;

use runtime_process::ExampleRuntime;
use serde_json::Value;

/// The example's observability section, off as it ships, and on.
const OBSERVABILITY_OFF: &str = "[runtime.observability]\nenabled = false";
const OBSERVABILITY_ON: &str = "[runtime.observability]\nenabled = true";

/// Long enough for several of the example's 1 s sample intervals on a loaded machine.
const RECORDED: Duration = Duration::from_secs(30);

/// The recorded values of `variable` in the history file `text`, in the order recorded: one JSON
/// object per line, `{"variable": ..., "value": {"kind": ..., "value": ...}, ...}`.
fn recorded(text: &str, variable: &str) -> Vec<i64> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|sample| sample["variable"] == variable)
        .filter_map(|sample| sample["value"]["value"].as_i64())
        .collect()
}

#[test]
fn the_historian_records_the_program_with_debugging_off() {
    // Specification 44 NX-14: the historian reads the scan's published copy of the program, as the
    // HMI and the OPC UA server do; specification 11: an enabled historian in `all` mode records
    // the program's values once per sample interval (the example's: 1 s) into its history file,
    // `history/historian.jsonl` in the project. `P1.Counter` rises by one every 50 ms scan, so its
    // recorded values rise from sample to sample. Catches a historian that reads only the
    // debugger's snapshot, which does not exist while debugging is off: it recorded nothing, and
    // `/metrics` reported no samples.
    let runtime = ExampleRuntime::new("memory_marker_counter")
        .mode("production")
        .debug_enabled(false)
        .edit_source("runtime.toml", OBSERVABILITY_OFF, OBSERVABILITY_ON)
        .start();
    let history = runtime.project().join("history").join("historian.jsonl");
    let counter = || {
        recorded(
            &std::fs::read_to_string(&history).unwrap_or_default(),
            "P1.Counter",
        )
    };
    runtime.wait_until(
        RECORDED,
        "three samples of P1.Counter in the history file",
        || counter().len() >= 3,
    );
    let samples = counter();
    assert!(
        samples.windows(2).all(|pair| pair[1] > pair[0]),
        "P1.Counter rises from sample to sample: {samples:?}"
    );
}
