//! Specification 11 §6.5: a variable bound to a marker (`%M`) starts from its initialized value at
//! a cold start and a cold restart, so the first start-of-cycle latch reads the declared value,
//! not a zero or pre-restart image (IEC 61131-3 Ed.3 §6.5.1, Figure 9).

use trust_runtime::harness::TestHarness;
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

const SOURCE: &str = r#"
PROGRAM Main
VAR
    Setpoint AT %MW0 : INT := 42;
    Count AT %MW2 : INT := 0;
    Seen : INT;
END_VAR
Seen := Setpoint;
Count := Count + 1;
END_PROGRAM
"#;

fn scan(harness: &mut TestHarness) {
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
}

/// Catches a runtime that latches the zero image into a marker variable before its first
/// cycle (Seen would be 0), or that keeps the pre-restart image after a cold restart (Count
/// would continue from 3 instead of starting again from its initial 0).
#[test]
fn marker_variables_start_from_their_initialized_values() {
    let mut harness = TestHarness::from_source(SOURCE).expect("compile and initialize");
    assert_eq!(
        harness
            .runtime()
            .io()
            .read(&IoAddress::parse("%MW0").unwrap())
            .unwrap(),
        Value::Word(42),
        "the marker holds the initialized value before the first cycle"
    );
    scan(&mut harness);
    assert_eq!(harness.get_output("Main.Seen"), Some(Value::Int(42)));
    scan(&mut harness);
    scan(&mut harness);
    assert_eq!(harness.get_output("Main.Count"), Some(Value::Int(3)));

    harness.restart(RestartMode::Cold).expect("cold restart");
    scan(&mut harness);
    assert_eq!(
        harness.get_output("Main.Count"),
        Some(Value::Int(1)),
        "a cold restart starts Count from 0, not from the image of the old run"
    );
}
