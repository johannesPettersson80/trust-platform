//! A debug image's Rust POU calls over their budgets on a computer (specification 48 RU-49;
//! specification 11, "Rust POU budgets"): the real `trust-runtime` runs a private copy of the
//! example `rust_function_block` with RAMP's budget lowered from 2 µs to 1 ns, so that every call
//! of it is over its budget. `events.tail` gives `rust_budget_exceeded` for the first call over
//! the budget and after it only for a call longer than every call before it, and `tasks.stats`
//! gives the POU's record.
//!
//! The test needs the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use serde_json::{json, Value};

use runtime_process::{ExampleRuntime, Runtime, CLIMB};

/// The events `rust_budget_exceeded` of `reply`, oldest first.
fn budget_events(reply: &Value) -> Vec<Value> {
    assert_eq!(reply["ok"], json!(true), "{reply}");
    let mut events: Vec<Value> = reply["result"]["events"]
        .as_array()
        .expect("the events")
        .iter()
        .filter(|event| event["type"] == json!("rust_budget_exceeded"))
        .cloned()
        .collect();
    // `events.tail` gives the newest first.
    events.reverse();
    events
}

/// RAMP's calls over its budget so far, as `tasks.stats` gives them; 0 before the first report.
fn calls_over_budget(runtime: &Runtime) -> u64 {
    let stats = runtime.control_request(json!({ "id": 9, "type": "tasks.stats" }));
    stats["result"]["rust_pous"][0]["over_budget"]
        .as_u64()
        .unwrap_or(0)
}

#[test]
fn a_rust_pou_over_its_budget_is_an_event_only_when_its_longest_call_grows() {
    // RU-49: RAMP runs in each activation of the example's 100 ms task, every call over its 1 ns;
    // the resource's scans are faster, so the test waits for RAMP's fifth call. The events name
    // RAMP with its budget and the call's time over it, each later one longer than the one before,
    // as only a call longer than every call before it is an event. tasks.stats, read after the
    // events, gives RAMP's budget, its calls over it, every call so at least five, and its longest
    // call, no shorter than the last event's. Catches a debug image that measures nothing, an event
    // for every call over the budget whose times then fall as often as they rise, the call's time
    // in the event's `time_ns`, and a tasks.stats without the POU.
    let runtime = ExampleRuntime::new("rust_function_block")
        .edit_source(
            "rust/src/lib.rs",
            "#[budget(\"2us\")]",
            "#[budget(\"1ns\")]",
        )
        .start();
    runtime.wait_until(CLIMB, "five calls of RAMP", || {
        calls_over_budget(&runtime) >= 5
    });
    let events = budget_events(&runtime.control_request(json!({
        "id": 1,
        "type": "events.tail",
        "params": { "limit": 1000 }
    })));
    assert!(!events.is_empty(), "no rust_budget_exceeded event");
    let mut longest = 0;
    for event in &events {
        assert_eq!(event["name"], json!("RAMP"), "{event}");
        assert_eq!(event["budget_ns"], json!(1), "{event}");
        let call = event["call_ns"].as_i64().expect("the call's time");
        assert!(call > 1, "a call over the budget: {event}");
        assert!(
            call > longest,
            "each event's call is longer than the one before: {events:?}"
        );
        longest = call;
        assert!(
            event["time_ns"].as_i64().is_some(),
            "the scan's time: {event}"
        );
    }
    let stats = runtime.control_request(json!({ "id": 2, "type": "tasks.stats" }));
    assert_eq!(stats["ok"], json!(true), "{stats}");
    let pous = stats["result"]["rust_pous"].as_array().expect("rust_pous");
    assert_eq!(pous.len(), 1, "{stats}");
    assert_eq!(pous[0]["name"], json!("RAMP"), "{stats}");
    assert_eq!(pous[0]["budget_ns"], json!(1), "{stats}");
    assert!(
        pous[0]["over_budget"]
            .as_u64()
            .expect("the calls over budget")
            >= 5,
        "{stats}"
    );
    assert!(
        pous[0]["longest_ns"].as_i64().expect("the longest call") >= longest,
        "{stats}"
    );
}
