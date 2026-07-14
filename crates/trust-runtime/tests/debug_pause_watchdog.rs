use std::thread;
use std::time::{Duration as WallDuration, Instant};

use trust_runtime::debug::{DebugStopReason, RuntimeEvent};
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ResourceRunner, ResourceState, StdClock};
use trust_runtime::value::Duration;
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};

const WATCHDOG_TIMEOUT_MS: i64 = 20;
const PAUSE_DWELL: WallDuration = WallDuration::from_millis(100);

fn wait_for_state(
    handle: &trust_runtime::scheduler::ResourceHandle<StdClock>,
    expected: ResourceState,
) {
    let deadline = Instant::now() + WallDuration::from_secs(2);
    while Instant::now() < deadline {
        if handle.state() == expected {
            return;
        }
        thread::sleep(WallDuration::from_millis(2));
    }
    panic!(
        "resource did not reach {expected:?}; state={:?}, error={:?}",
        handle.state(),
        handle.last_error()
    );
}

fn watchdog_policy() -> WatchdogPolicy {
    WatchdogPolicy {
        enabled: true,
        timeout: Duration::from_millis(WATCHDOG_TIMEOUT_MS),
        action: WatchdogAction::Halt,
    }
}

fn counter_runtime() -> (trust_runtime::Runtime, trust_runtime::debug::DebugControl) {
    let source = r#"
PROGRAM Main
VAR
    counter : DINT := 0;
END_VAR
counter := counter + DINT#1;
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile pause/watchdog fixture");
    let debug = harness.runtime_mut().enable_debug();
    (harness.into_runtime(), debug)
}

#[test]
fn statement_pause_longer_than_watchdog_does_not_fault_resource() {
    let (runtime, debug) = counter_runtime();
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    debug.set_stop_sender(stop_tx);
    debug.pause();

    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    runner.runtime_mut().set_watchdog_policy(watchdog_policy());
    let mut handle = runner.spawn("debug-pause-watchdog").expect("spawn runtime");

    let stop = stop_rx
        .recv_timeout(WallDuration::from_secs(2))
        .expect("runtime must stop at the requested statement boundary");
    assert_eq!(stop.reason, DebugStopReason::Pause);

    thread::sleep(PAUSE_DWELL);
    assert_eq!(handle.state(), ResourceState::Running);
    assert_eq!(handle.last_error(), None);

    debug.continue_run();
    thread::sleep(WallDuration::from_millis(60));
    assert_eq!(
        handle.state(),
        ResourceState::Running,
        "operator dwell at a statement pause must not become watchdog execution time; error={:?}",
        handle.last_error()
    );
    assert_eq!(handle.last_error(), None);

    handle.stop();
    debug.continue_run();
    handle.join().expect("join runtime");
}

#[test]
fn resource_pause_between_cycles_starts_no_scan_or_watchdog_interval() {
    let (runtime, debug) = counter_runtime();
    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    runner.runtime_mut().set_watchdog_policy(watchdog_policy());
    let mut handle = runner
        .spawn("resource-pause-watchdog")
        .expect("spawn runtime");
    let control = handle.control();

    wait_for_state(&handle, ResourceState::Running);
    control.pause().expect("pause resource");
    wait_for_state(&handle, ResourceState::Paused);
    let _ = debug.drain_runtime_events();

    thread::sleep(PAUSE_DWELL);
    assert_eq!(handle.state(), ResourceState::Paused);
    assert_eq!(handle.last_error(), None);
    assert!(
        debug.drain_runtime_events().iter().all(|event| !matches!(
            event,
            RuntimeEvent::CycleStart { .. } | RuntimeEvent::CycleEnd { .. }
        )),
        "a resource pause between cycles must not begin another scan"
    );

    control.resume().expect("resume resource");
    wait_for_state(&handle, ResourceState::Running);
    thread::sleep(WallDuration::from_millis(40));
    assert_eq!(handle.state(), ResourceState::Running);
    assert_eq!(handle.last_error(), None);

    handle.stop();
    handle.join().expect("join runtime");
}
