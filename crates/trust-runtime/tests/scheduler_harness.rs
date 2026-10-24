use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ManualClock, ResourceRunner};
use trust_runtime::value::Duration;

#[test]
fn deterministic_clock() {
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

    clock.set_time(Duration::from_millis(0));
    runner.tick().unwrap();
    assert_eq!(runner.program().current_time(), Duration::from_millis(0));

    clock.advance(Duration::from_millis(5));
    runner.tick().unwrap();
    assert_eq!(runner.program().current_time(), Duration::from_millis(5));

    assert_eq!(clock.sleep_calls(), 0);
}
