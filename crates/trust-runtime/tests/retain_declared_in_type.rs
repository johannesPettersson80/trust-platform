//! IEC 61131-3 Ed.3 §6.5.6.1 and §6.5.6.2, specification 10 §4.8: a variable that a function
//! block type declares `RETAIN` keeps its value over a warm restart in every instance, whether it
//! is a local, an input or an output; one it declares `NON_RETAIN` starts over, even in an
//! instance declared `RETAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

const SOURCE: &str = r#"
FUNCTION_BLOCK Counter
VAR_INPUT RETAIN
    Preset : DINT;
END_VAR
VAR_INPUT NON_RETAIN
    Increment : DINT := 1;
END_VAR
VAR_OUTPUT RETAIN
    Count : DINT;
END_VAR
VAR_OUTPUT NON_RETAIN
    Scans : DINT;
END_VAR
VAR RETAIN
    Kept : DINT;
END_VAR
    Count := Count + Increment;
    Scans := Scans + 1;
    Kept := Kept + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Plain : Counter;
    Arm : BOOL;
END_VAR
VAR RETAIN
    Whole : Counter;
END_VAR
    IF Arm THEN
        Plain.Preset := 7;
        Whole.Increment := 5;
    END_IF;
    Plain();
    Whole();
END_PROGRAM
"#;

fn value(harness: &TestHarness, path: &str) -> Value {
    harness
        .get_output(path)
        .unwrap_or_else(|| panic!("{path:?} exists"))
}

fn scan(harness: &mut TestHarness) {
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
}

#[test]
fn a_warm_restart_keeps_what_the_type_declares_retain_and_resets_what_it_declares_non_retain() {
    let mut harness = TestHarness::from_source(SOURCE).expect("compile and initialize");
    harness.set_input("Arm", Value::Bool(true));
    scan(&mut harness);
    harness.set_input("Arm", Value::Bool(false));
    scan(&mut harness);
    harness.restart(RestartMode::Warm).expect("warm restart");
    // Before the first scan after the restart: only the retained values remain.
    let plain = |name: &str| value(&harness, &format!("Main.Plain.{name}"));
    assert_eq!(
        plain("Preset"),
        Value::DInt(7),
        "a RETAIN input of a plain instance is kept"
    );
    assert_eq!(
        plain("Count"),
        Value::DInt(2),
        "a RETAIN output of a plain instance is kept"
    );
    assert_eq!(
        plain("Kept"),
        Value::DInt(2),
        "a RETAIN local of a plain instance is kept"
    );
    assert_eq!(
        plain("Scans"),
        Value::DInt(0),
        "a NON_RETAIN output starts over"
    );
    let whole = |name: &str| value(&harness, &format!("Main.Whole.{name}"));
    assert_eq!(
        whole("Increment"),
        Value::DInt(1),
        "a NON_RETAIN input starts over in a RETAIN instance"
    );
    assert_eq!(
        whole("Scans"),
        Value::DInt(0),
        "a NON_RETAIN output starts over in a RETAIN instance"
    );
    assert_eq!(
        whole("Count"),
        Value::DInt(10),
        "the rest of a RETAIN instance is kept"
    );
}
