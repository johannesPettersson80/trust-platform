use std::time::Duration as StdDuration;

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ManualClock, ResourceRunner, ResourceState};
use trust_runtime::value::Duration;

#[test]
fn fault_stops_resource() {
    let source = r#"
PROGRAM Main
VAR
    x : INT := 0;
END_VAR
x := 1 / 0;
END_PROGRAM
"#;

    let runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let clock = ManualClock::new();
    let runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(10));
    let mut handle = runner.spawn("faulty-resource").unwrap();

    let start = std::time::Instant::now();
    loop {
        if handle.state() == ResourceState::Faulted {
            break;
        }
        if start.elapsed() > StdDuration::from_secs(10) {
            panic!("resource did not fault in time");
        }
        std::thread::yield_now();
    }

    assert!(matches!(
        handle.last_error(),
        Some(RuntimeError::DivisionByZero)
    ));

    handle.stop();
    handle.join().unwrap();
}
