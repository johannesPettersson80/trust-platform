#![cfg(feature = "full")]
//! Operations while the debugger holds the scan (specification 11 section 6.9, "Operations while the
//! debugger holds the scan").
//!
//! A hold takes effect at the next statement boundary. These tests run the scan on a second thread, as
//! the resource does, and observe it from the first: a single thread that issued the scan under a hold
//! would wait for itself. Each test releases the hold and joins the scan thread before it asserts, so a
//! failed assertion never leaves a thread waiting. The stop tests at the end of the file release the
//! hold only when the stop request itself failed to end the resource.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as WallDuration, Instant};

use trust_runtime::debug::{DebugControl, DebugStop, DebugStopReason, RuntimeEvent};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoDriver, IoSafeState};
use trust_runtime::retain::RetainStore;
use trust_runtime::scheduler::{ResourceHandle, ResourceRunner, ResourceState, StdClock};
use trust_runtime::value::{Duration, Value};
use trust_runtime::{RestartMode, RetainSnapshot, Runtime};

const WAIT: WallDuration = WallDuration::from_secs(5);
/// How long the restart test keeps the hold, to show that the request is not taken meanwhile.
const DWELL: WallDuration = WallDuration::from_millis(150);

/// `gate` and `late` are written only by queued writes. The body counts its first statement in
/// `ticks`, and `lamp` is the output image that the second statement sets.
const SOURCE: &str = r#"
VAR_GLOBAL
    gate : DINT := DINT#0;
    late : DINT := DINT#0;
    ticks : DINT := DINT#0;
    lamp AT %QX0.0 : BOOL := FALSE;
END_VAR

PROGRAM Main
ticks := ticks + DINT#1;
lamp := gate > DINT#0;
END_PROGRAM
"#;

fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(WallDuration::from_millis(2));
    }
    condition()
}

/// Spec 11 6.9: a scan that starts while a hold is already requested applies its boundary writes,
/// then waits before the first statement of its first program. The waiting scan has run no statement,
/// and releasing the hold continues that same scan: it completes once, commits its outputs and
/// advances the cycle number by one. A write queued during the hold waits for the next scan boundary.
#[test]
fn scan_started_under_a_hold_applies_its_boundary_writes_then_waits_before_its_first_statement() {
    let mut harness = TestHarness::from_source(SOURCE).expect("fixture compiles");
    let debug = harness.runtime_mut().enable_debug();
    let (stop_tx, stop_rx) = mpsc::channel();
    debug.set_stop_sender(stop_tx);
    debug.enqueue_global_write("gate", Value::DInt(3));
    debug.pause();

    let mut runtime = harness.into_runtime();
    let scan = thread::spawn(move || {
        let result = runtime.execute_cycle();
        (runtime, result)
    });

    let stop = stop_rx.recv_timeout(WAIT);
    let snapshot = debug.snapshot();
    let held = debug.is_paused();
    let events_during_hold = debug.drain_runtime_events();
    debug.enqueue_global_write("late", Value::DInt(9));
    debug.continue_run();
    let (mut runtime, result) = scan.join().expect("the scan thread ends");
    let events_after_release = debug.drain_runtime_events();

    let stop = stop.expect("the scan waits at a statement boundary");
    assert_eq!(stop.reason, DebugStopReason::Pause);
    assert!(held, "the debugger holds the scan while it waits");
    assert!(
        events_during_hold
            .iter()
            .any(|event| matches!(event, RuntimeEvent::CycleStart { .. })),
        "the waiting scan has begun"
    );
    assert!(
        !events_during_hold
            .iter()
            .any(|event| matches!(event, RuntimeEvent::CycleEnd { .. })),
        "the waiting scan has not ended"
    );
    assert_eq!(
        events_after_release
            .iter()
            .filter(|event| matches!(event, RuntimeEvent::CycleEnd { .. }))
            .count(),
        1,
        "releasing the hold ends that same scan, once"
    );
    let snapshot = snapshot.expect("the hold publishes the state in which the scan waits");
    assert_eq!(
        snapshot.storage.get_global("gate"),
        Some(&Value::DInt(3)),
        "the boundary write is applied before the scan waits"
    );
    assert_eq!(
        snapshot.storage.get_global("ticks"),
        Some(&Value::DInt(0)),
        "the waiting scan has run no statement"
    );

    assert_eq!(result, Ok(()), "releasing the hold continues the same scan");
    assert_eq!(runtime.cycle_counter(), 1);
    assert_eq!(
        runtime.storage().get_global("ticks"),
        Some(&Value::DInt(1)),
        "the body ran once"
    );
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%QX0.0").expect("address parses")),
        Ok(Value::Bool(true)),
        "the continued scan commits its outputs"
    );
    assert_eq!(
        runtime.storage().get_global("late"),
        Some(&Value::DInt(0)),
        "a write queued during the hold is not applied by the scan that waits"
    );
    runtime
        .execute_cycle()
        .expect("the next scan completes without a hold");
    assert_eq!(
        runtime.storage().get_global("late"),
        Some(&Value::DInt(9)),
        "the write queued during the hold is applied at the next scan boundary"
    );
    assert_eq!(runtime.cycle_counter(), 2);
}

/// Spec 11 6.9: the scan thread takes restart requests only between scans, so a request made while
/// it waits in a hold waits too. It is taken after the hold is released and the held scan has
/// completed, and the restart then succeeds.
#[test]
fn restart_request_waits_for_the_hold_and_is_taken_after_the_held_scan_completes() {
    let mut harness = TestHarness::from_source(SOURCE).expect("fixture compiles");
    let debug = harness.runtime_mut().enable_debug();
    let (stop_tx, stop_rx) = mpsc::channel();
    debug.set_stop_sender(stop_tx);
    debug.pause();
    let signal: Arc<Mutex<Option<RestartMode>>> = Arc::new(Mutex::new(None));
    let runner = ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        Duration::from_millis(1),
    )
    .with_restart_signal(Arc::clone(&signal));
    let mut handle = runner.spawn("hold-restart").expect("spawn the resource");

    let stop = stop_rx.recv_timeout(WAIT);
    *signal.lock().expect("restart signal") = Some(RestartMode::Cold);
    thread::sleep(DWELL);
    let pending_during_hold = signal.lock().expect("restart signal").is_some();
    let state_during_hold = handle.state();
    let error_during_hold = handle.last_error();

    debug.continue_run();
    let taken = wait_until(|| signal.lock().expect("restart signal").is_none());
    let running_again = wait_until(|| handle.state() == ResourceState::Running);
    let error_after = handle.last_error();
    handle.stop();
    debug.continue_run();
    handle.join().expect("the resource thread ends");

    let stop = stop.expect("the scan waits at a statement boundary");
    assert_eq!(stop.reason, DebugStopReason::Pause);
    assert!(
        pending_during_hold,
        "the request is not taken while the scan waits in the hold"
    );
    assert_eq!(
        state_during_hold,
        ResourceState::Running,
        "a hold is not a resource state"
    );
    assert_eq!(error_during_hold, None);
    assert!(
        taken,
        "the request is taken after the hold ends and the held scan completes"
    );
    assert!(
        running_again && error_after.is_none(),
        "the restart succeeds"
    );
}

/// `kept` is retained, so the checkpoint that the stop saves shows whether the suspended scan ran to its
/// end. `lamp` is the output that the stop policy sets to FALSE after the scan has set it to TRUE.
const STOP_SOURCE: &str = r#"
VAR_GLOBAL RETAIN
    kept : DINT := DINT#7;
END_VAR

VAR_GLOBAL
    lamp AT %QX0.0 : BOOL := FALSE;
END_VAR

PROGRAM Main
kept := kept + DINT#1;
lamp := TRUE;
END_PROGRAM
"#;

/// An output driver that records every image the scan hands it.
struct RecordingDriver {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl IoDriver for RecordingDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.writes
            .lock()
            .expect("driver writes lock")
            .push(outputs.to_vec());
        Ok(())
    }
}

/// A retain store that records every checkpoint it is given.
struct RecordingStore {
    saved: Arc<Mutex<Vec<RetainSnapshot>>>,
}

impl RetainStore for RecordingStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Ok(RetainSnapshot::default())
    }

    fn store(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        self.saved
            .lock()
            .expect("store lock")
            .push(snapshot.clone());
        Ok(())
    }
}

/// What the stop tests read after the resource has ended.
struct Probes {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    saved: Arc<Mutex<Vec<RetainSnapshot>>>,
}

impl Probes {
    /// The images the output driver received, last one last.
    fn last_image(&self) -> Option<Vec<u8>> {
        self.writes
            .lock()
            .expect("driver writes lock")
            .last()
            .cloned()
    }

    /// The retained `kept` in the last checkpoint that the store was given.
    fn last_kept(&self) -> Option<Value> {
        let saved = self.saved.lock().expect("store lock");
        saved
            .last()
            .and_then(|snapshot| snapshot.values().get("kept").cloned())
    }
}

/// The runtime of the stop tests with a debug control, a recording output driver, the stop policy
/// `lamp := FALSE` and a recording retain store.
fn stop_fixture() -> (Runtime, DebugControl, Probes) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let saved = Arc::new(Mutex::new(Vec::new()));
    let mut harness = TestHarness::from_source(STOP_SOURCE).expect("fixture compiles");
    let runtime = harness.runtime_mut();
    let debug = runtime.enable_debug();
    runtime.add_io_driver(
        "recorder",
        Box::new(RecordingDriver {
            writes: Arc::clone(&writes),
        }),
    );
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((
        IoAddress::parse("%QX0.0").expect("address parses"),
        Value::Bool(false),
    ));
    runtime.set_io_safe_state(safe_state);
    runtime.set_retain_store(
        Some(Box::new(RecordingStore {
            saved: Arc::clone(&saved),
        })),
        None,
    );
    (harness.into_runtime(), debug, Probes { writes, saved })
}

/// A resource whose scan the debugger holds at its first statement.
struct Held {
    handle: ResourceHandle<StdClock>,
    debug: DebugControl,
    probes: Probes,
    /// The stop event of the hold, or the timeout when the scan never waited.
    hold: Result<DebugStop, mpsc::RecvTimeoutError>,
    /// Keeps the channel of the stop events open.
    _events: mpsc::Receiver<DebugStop>,
}

impl Held {
    /// Waits until the resource has ended `Stopped` and joins its thread. When the resource does not
    /// end, the hold is released by hand so that no test leaves a scan thread waiting. Returns whether
    /// the stop request alone ended the resource.
    fn ended_by_stop(&mut self) -> bool {
        let ended = wait_until(|| self.handle.state() == ResourceState::Stopped);
        if !ended {
            self.debug.continue_run();
        }
        self.handle.join().expect("the resource thread ends");
        ended
    }
}

/// Starts the resource with a debug-mode pause requested before it starts, so that the scan waits before
/// its first statement, and returns once the stop event of the hold has arrived.
fn start_held(runtime: Runtime, debug: DebugControl, probes: Probes, name: &str) -> Held {
    let (stop_tx, events) = mpsc::channel();
    debug.set_stop_sender(stop_tx);
    debug.pause();
    let handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
        .spawn(name)
        .expect("spawn the resource");
    let hold = events.recv_timeout(WAIT);
    Held {
        handle,
        debug,
        probes,
        hold,
        _events: events,
    }
}

fn spawn_held(name: &str) -> Held {
    let (runtime, debug, probes) = stop_fixture();
    start_held(runtime, debug, probes, name)
}

/// Spec 11 6.9 ("Operations while the debugger holds the scan") and spec 13 "Pause": a stop request is
/// independent of the hold. The scan waits in a debug-mode pause that was requested before the resource
/// started, and nobody resumes. The stop alone releases the hold. The scan that the hold suspended runs
/// to its end (it counted `kept` up and set the lamp), and the resource takes its ordinary stop path:
/// the stop output is the last image the driver received, the checkpoint is saved, and the resource
/// ends `Stopped`. Pre-fix the scan thread waited for a `resume` that never came.
#[test]
fn stop_request_during_a_hold_ends_the_resource_through_the_normal_stop_path() {
    let mut held = spawn_held("stop-during-hold");
    let held_by_debugger = held.debug.is_paused();

    held.handle.stop();
    let ended = held.ended_by_stop();

    let stop = held.hold.expect("the scan waits at a statement boundary");
    assert_eq!(stop.reason, DebugStopReason::Pause);
    assert!(
        held_by_debugger,
        "the debugger holds the scan when the stop is requested"
    );
    assert!(
        ended,
        "a stop request ends a resource whose scan the debugger holds, without a resume"
    );
    assert_eq!(held.handle.state(), ResourceState::Stopped);
    assert_eq!(held.handle.last_error(), None);
    assert_eq!(
        held.probes.last_image(),
        Some(vec![0u8]),
        "the stop output is the last image that the driver received"
    );
    assert_eq!(
        held.probes.last_kept(),
        Some(Value::DInt(8)),
        "the stop saved the retained values after the suspended scan had run to its end"
    );
}

/// The control `shutdown` and the termination signals stop through `ResourceControl`. A debugger client
/// may pause again right after the stop request, before the scan thread has woken. The stop keeps the
/// debugger from holding the scan again, so the resource still ends without a resume.
#[test]
fn stop_through_the_control_handle_is_not_held_again_by_a_pause_that_follows() {
    let mut held = spawn_held("control-stop-during-hold");
    let control = held.handle.control();

    control.stop();
    held.debug.pause();
    let ended = held.ended_by_stop();

    let stop = held.hold.expect("the scan waits at a statement boundary");
    assert_eq!(stop.reason, DebugStopReason::Pause);
    assert!(
        ended,
        "a pause that follows the stop request does not hold the scan again"
    );
    assert_eq!(held.handle.state(), ResourceState::Stopped);
    assert_eq!(held.handle.last_error(), None);
    assert_eq!(held.probes.last_image(), Some(vec![0u8]));
    assert_eq!(held.probes.last_kept(), Some(Value::DInt(8)));
}

/// A stop ends the hold for the resource it stops only. The next resource thread that starts on the same
/// debugger is held again: the debugger is not left inert by the earlier stop.
#[test]
fn resource_thread_started_after_a_stop_is_held_by_the_debugger_again() {
    let mut first = spawn_held("first-resource");
    first.handle.stop();
    let first_ended = first.ended_by_stop();

    let (mut runtime, _, probes) = stop_fixture();
    runtime.set_debug_control(first.debug.clone());
    let mut second = start_held(runtime, first.debug.clone(), probes, "second-resource");
    let second_held = second.hold.is_ok();
    second.handle.stop();
    let second_ended = second.ended_by_stop();

    assert!(first_ended, "the first resource ends by its stop");
    assert!(
        second_held,
        "the second resource thread is held by the debugger again"
    );
    assert!(
        second_ended,
        "the stop request ends the second held resource as well"
    );
    assert_eq!(second.handle.state(), ResourceState::Stopped);
    assert_eq!(second.probes.last_image(), Some(vec![0u8]));
}
