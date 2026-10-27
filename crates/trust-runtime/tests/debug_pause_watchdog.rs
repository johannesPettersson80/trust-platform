//! A debugger hold and a resource pause are no watchdog execution time (specification 11 §6.6,
//! §6.9), on the debugger of compiled programs (specification 44 §13.2).
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::thread;
use std::time::{Duration as WallDuration, Instant};

use serde_json::Value as Json;
use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ResourceRunner, ResourceState, StdClock};
use trust_runtime::value::Duration;
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};
use trust_runtime::{CompiledDebugger, CompiledRuntime};
use trust_runtime_services::live_values::io_requests::IoSnapshotRead;
use trust_runtime_services::protocol::ControlRequest;

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

/// The reason of the next stop the debugger records within 2 s; `None` when none arrives.
fn next_stop(debugger: &CompiledDebugger) -> Option<String> {
    let request = ControlRequest {
        id: 1,
        r#type: "debug.stops".to_string(),
        params: None,
        auth: None,
        request_id: None,
    };
    let deadline = Instant::now() + WallDuration::from_secs(2);
    loop {
        let answer: Json = serde_json::to_value(debugger.answer(&request, IoSnapshotRead::Missing))
            .expect("a response serializes");
        if let Some(stop) = answer["result"]["stops"]
            .as_array()
            .and_then(|stops| stops.first())
        {
            return stop["reason"].as_str().map(str::to_string);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(WallDuration::from_millis(2));
    }
}

fn counter_runtime() -> (CompiledRuntime, DebugControl, CompiledDebugger) {
    let source = r#"
PROGRAM Main
VAR
    counter : DINT := 0;
END_VAR
counter := counter + DINT#1;
END_PROGRAM
"#;
    let mut runtime = TestHarness::from_source(source)
        .expect("compile pause/watchdog fixture")
        .into_runtime();
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    let debugger = debug.compiled().expect("the compiled program's debugger");
    (runtime, debug, debugger)
}

#[test]
fn statement_pause_longer_than_watchdog_does_not_fault_resource() {
    let (runtime, _debug, debugger) = counter_runtime();
    debugger.pause();

    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    runner.program_mut().set_watchdog_policy(watchdog_policy());
    let mut handle = runner.spawn("debug-pause-watchdog").expect("spawn runtime");

    assert_eq!(
        next_stop(&debugger).as_deref(),
        Some("pause"),
        "runtime must stop at the requested statement boundary"
    );

    thread::sleep(PAUSE_DWELL);
    assert_eq!(handle.state(), ResourceState::Running);
    assert_eq!(handle.last_error(), None);

    debugger.resume();
    thread::sleep(WallDuration::from_millis(60));
    assert_eq!(
        handle.state(),
        ResourceState::Running,
        "operator dwell at a statement pause must not become watchdog execution time; error={:?}",
        handle.last_error()
    );
    assert_eq!(handle.last_error(), None);

    handle.stop();
    debugger.resume();
    handle.join().expect("join runtime");
}

#[test]
fn resource_pause_between_cycles_starts_no_scan_or_watchdog_interval() {
    let (runtime, debug, _debugger) = counter_runtime();
    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    runner.program_mut().set_watchdog_policy(watchdog_policy());
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

#[test]
fn supervisor_does_not_trip_during_a_debugger_hold_longer_than_the_timeout() {
    let (runtime, _debug, debugger) = counter_runtime();
    debugger.pause();

    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    runner.program_mut().set_watchdog_policy(watchdog_policy());
    let mut handle = runner
        .spawn("debug-hold-supervisor")
        .expect("spawn runtime");

    assert_eq!(
        next_stop(&debugger).as_deref(),
        Some("pause"),
        "runtime must stop at the requested statement boundary"
    );

    // The hold lasts five watchdog timeouts. The supervisor finds the scan thread inside the hold
    // and suspends its check, so the hold is never declared a hang.
    thread::sleep(PAUSE_DWELL);
    assert!(!handle.is_hung(), "a debugger hold is not a hang");
    assert_eq!(handle.state(), ResourceState::Running);
    assert_eq!(handle.last_error(), None);

    debugger.resume();
    thread::sleep(WallDuration::from_millis(60));
    assert!(!handle.is_hung());
    assert_eq!(handle.state(), ResourceState::Running);
    assert_eq!(handle.last_error(), None);

    handle.stop();
    debugger.resume();
    handle.join().expect("join runtime");
}
