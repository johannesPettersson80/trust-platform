use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

#[test]
fn from_source() {
    let source = r#"
        PROGRAM Demo
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.assert_eq("count", 0i32);
    harness.cycle();
    harness.assert_eq("count", 1i32);
}

#[test]
fn io_by_name() {
    let source = r#"
        PROGRAM Copy
        VAR
            input: BOOL := FALSE;
            output: BOOL := FALSE;
        END_VAR
        output := input;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.set_input("input", true);
    harness.cycle();
    assert_eq!(harness.get_output("output"), Some(Value::Bool(true)));
}

#[test]
fn io_by_address() {
    let source = r#"
        PROGRAM Io
        VAR
            in: BOOL := FALSE;
            out: BOOL := FALSE;
        END_VAR
        out := in;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.bind_direct("in", "%IX0.0").unwrap();
    harness.bind_direct("out", "%QX0.1").unwrap();

    harness
        .set_direct_input("%IX0.0", Value::Bool(true))
        .unwrap();
    harness.cycle();
    let out = harness.get_direct_output("%QX0.1").unwrap();
    assert_eq!(out, Value::Bool(true));
}

#[test]
fn run_controls() {
    let source = r#"
        PROGRAM Counter
        VAR
            count: DINT := 0;
        END_VAR
        count := count + 1;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.run_cycles(3);
    harness.assert_eq("count", 3i32);

    let results = harness.run_until(|runtime| {
        let Some(Value::Instance(id)) = runtime.storage().get_global("Counter") else {
            return false;
        };
        runtime.storage().get_instance_var(*id, "count") == Some(&Value::DInt(5))
    });
    assert_eq!(results.len(), 2);
    assert_eq!(harness.current_time(), Duration::ZERO);
    assert_eq!(harness.cycle_count(), 5);
}

#[test]
fn run_until_returns_immediately_when_condition_is_already_true() {
    let source = r#"
        PROGRAM Demo
        END_PROGRAM
    "#;
    let mut harness = TestHarness::from_source(source).unwrap();
    let results = harness.run_until(|_| true);
    assert!(results.is_empty());
    assert_eq!(harness.cycle_count(), 0);
}

#[test]
#[should_panic(expected = "run_until exceeded 3 cycles")]
fn run_until_max_panics_when_limit_is_exceeded() {
    let source = r#"
        PROGRAM Demo
        END_PROGRAM
    "#;
    let mut harness = TestHarness::from_source(source).unwrap();
    let _ = harness.run_until_max(|_| false, 3);
}
