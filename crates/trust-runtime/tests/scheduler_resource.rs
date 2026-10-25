//! A resource runs on a thread of its own and reports its scans.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::time::Duration as StdDuration;

use trust_runtime::debug::RuntimeEvent;
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ManualClock, ResourceRunner};
use trust_runtime::value::Duration;

#[test]
fn resource_runs_in_thread() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let debug = trust_runtime::debug::DebugControl::new();
    runtime.set_debug_control(debug.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    debug.set_runtime_sender(tx);

    let clock = ManualClock::new();
    let runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(10));
    let mut handle = runner.spawn("resource-thread").unwrap();

    assert_ne!(handle.thread_id(), std::thread::current().id());

    let event = rx.recv_timeout(StdDuration::from_millis(50)).unwrap();
    assert!(matches!(
        event,
        RuntimeEvent::CycleStart { .. } | RuntimeEvent::CycleEnd { .. }
    ));

    handle.stop();
    handle.join().unwrap();
}
