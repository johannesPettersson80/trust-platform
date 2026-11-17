//! The standard function block `SERVICE_FRESH`, truST's freshness of a Rust service's heartbeat
//! (specification 08 §5A; specification 48 RU-63), compiled and run by the harness on the task's
//! logical time.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

/// A program that gates on a heartbeat with a timeout of one second.
const PROGRAM: &str = "PROGRAM Main\n\
    VAR fresh : SERVICE_FRESH; heartbeat : UDINT; ok : BOOL; age : TIME; END_VAR\n\
    fresh(HEARTBEAT := heartbeat, TIMEOUT := T#1s, FRESH => ok, AGE => age);\n\
    END_PROGRAM";

/// Runs one cycle `after` milliseconds after the previous one, with the heartbeat `heartbeat`, and
/// returns `ok` and `age` in milliseconds.
fn cycle(harness: &mut TestHarness, after: i64, heartbeat: u32) -> (Value, Value) {
    harness.set_input("heartbeat", Value::UDInt(heartbeat));
    harness.advance_time(Duration::from_millis(after));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    (
        harness.get_output("ok").expect("ok"),
        harness.get_output("age").expect("age"),
    )
}

fn fresh(ok: bool, age_ms: i64) -> (Value, Value) {
    (Value::Bool(ok), Value::Time(Duration::from_millis(age_ms)))
}

#[test]
fn service_fresh_follows_a_heartbeat_through_the_scan() {
    // Specification 08 §5A, worked by hand, one call per cycle: the first cycle keeps heartbeat 0
    // without a change (not fresh, AGE 0); 300 ms later, unchanged, AGE 300 ms and still not fresh;
    // heartbeat 1 at 500 ms counts a change, fresh with AGE 0; unchanged until 1500 ms, AGE exactly
    // the 1 s timeout, still fresh; 10 ms later it is stale. A cold restart starts the block again:
    // its first call keeps the heartbeat it finds without a change, so the program waits for the
    // service's next heartbeat. Catches a block the build does not know, a first call counted as a
    // change, a timeout bound that is not inclusive, and a block that a restart leaves fresh.
    let mut harness = TestHarness::from_source(PROGRAM).expect("the program compiles");
    assert_eq!(cycle(&mut harness, 0, 0), fresh(false, 0));
    assert_eq!(cycle(&mut harness, 300, 0), fresh(false, 300));
    assert_eq!(cycle(&mut harness, 200, 1), fresh(true, 0));
    assert_eq!(cycle(&mut harness, 1000, 1), fresh(true, 1000));
    assert_eq!(cycle(&mut harness, 10, 1), fresh(false, 1010));
    harness
        .restart(RestartMode::Cold)
        .expect("the cold restart");
    assert_eq!(cycle(&mut harness, 10, 1), fresh(false, 0));
    assert_eq!(cycle(&mut harness, 10, 2), fresh(true, 0));
}
