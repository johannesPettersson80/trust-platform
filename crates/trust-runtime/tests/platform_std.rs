use std::time::Duration as StdDuration;

use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{Clock, ManualClock, ResourceRunner, StdClock};
use trust_runtime::value::{Duration, Value};

#[test]
fn monotonic_time() {
    let clock = StdClock::new();
    let t1 = clock.now();
    std::thread::sleep(StdDuration::from_millis(2));
    let t2 = clock.now();
    assert!(t2.as_nanos() >= t1.as_nanos());
}

#[test]
fn sleep_not_in_tests() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

    let runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));

    runner.tick().unwrap();
    assert_eq!(clock.sleep_calls(), 0);
}

#[test]
fn time_builtin_uses_runtime_clock() {
    let source = r#"
PROGRAM Main
VAR
    stamp : TIME;
END_VAR
stamp := TIME();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.advance_time(Duration::from_millis(123));
    harness.cycle();

    harness.assert_eq("stamp", Value::Time(Duration::from_millis(123)));
}
