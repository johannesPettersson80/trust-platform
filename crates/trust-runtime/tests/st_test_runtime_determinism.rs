use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ManualClock, ResourceRunner};
use trust_runtime::value::Duration;

fn run_once_with_manual_clock() -> Result<(), RuntimeError> {
    let source = r#"
PROGRAM DeterministicAssertions
VAR
    Counter : INT := 0;
END_VAR
Counter := Counter + 1;
ASSERT_EQUAL(INT#1, Counter);
END_PROGRAM
"#;

    let runtime = TestHarness::from_source(source)
        .expect("compile deterministic assertion source")
        .into_runtime();
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));

    clock.set_time(Duration::from_millis(0));
    runner.tick()?;

    clock.advance(Duration::from_millis(5));
    runner.tick()
}

#[test]
fn assertion_result_is_deterministic_with_manual_clock() {
    let first = run_once_with_manual_clock().unwrap_err();
    let second = run_once_with_manual_clock().unwrap_err();

    match (first, second) {
        (RuntimeError::AssertionFailed(left), RuntimeError::AssertionFailed(right)) => {
            assert_eq!(left, right);
            assert!(left.contains("ASSERT_EQUAL"));
            assert!(left.contains("expected"));
            assert!(left.contains("actual"));
        }
        (left, right) => panic!("expected assertion failures, got '{left}' and '{right}'"),
    }
}
