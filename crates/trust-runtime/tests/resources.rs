//! Resources run on threads of their own.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::mpsc::Receiver;
use std::time::Duration as StdDuration;

use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ManualClock, ResourceRunner};
use trust_runtime::value::Duration;

fn wait_for_cycle(rx: &Receiver<RuntimeEvent>, target: u64) {
    let start = std::time::Instant::now();
    loop {
        let event = rx
            .recv_timeout(StdDuration::from_millis(100))
            .expect("timed out waiting for runtime event");
        let cycle = match event {
            RuntimeEvent::CycleStart { cycle, .. } | RuntimeEvent::CycleEnd { cycle, .. } => cycle,
            _ => continue,
        };
        if cycle >= target {
            break;
        }
        if start.elapsed() > StdDuration::from_millis(500) {
            panic!("timed out waiting for cycle {target}");
        }
    }
}

#[test]
fn multiple_resources() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

    let mut runtime_a = TestHarness::from_source(source).unwrap().into_runtime();
    let debug_a = DebugControl::new();
    runtime_a.set_debug_control(debug_a.clone());
    let (tx_a, rx_a) = std::sync::mpsc::channel();
    debug_a.set_runtime_sender(tx_a);

    let mut runtime_b = TestHarness::from_source(source).unwrap().into_runtime();
    let debug_b = DebugControl::new();
    runtime_b.set_debug_control(debug_b.clone());
    let (tx_b, rx_b) = std::sync::mpsc::channel();
    debug_b.set_runtime_sender(tx_b);

    let clock_a = ManualClock::new();
    let clock_b = ManualClock::new();

    let runner_a = ResourceRunner::new(runtime_a, clock_a.clone(), Duration::from_millis(10));
    let runner_b = ResourceRunner::new(runtime_b, clock_b.clone(), Duration::from_millis(10));

    let mut handle_a = runner_a.spawn("res-a").unwrap();
    let mut handle_b = runner_b.spawn("res-b").unwrap();

    assert_ne!(handle_a.thread_id(), handle_b.thread_id());
    assert_ne!(handle_a.thread_id(), std::thread::current().id());
    assert_ne!(handle_b.thread_id(), std::thread::current().id());

    wait_for_cycle(&rx_a, 0);
    wait_for_cycle(&rx_b, 0);

    handle_a.stop();
    handle_b.stop();
    handle_a.join().unwrap();
    handle_b.join().unwrap();
}
