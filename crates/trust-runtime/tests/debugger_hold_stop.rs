#![cfg(feature = "full")]
//! A resource stop ends a debugger hold (specification 11 section 6.9, "Operations while the
//! debugger holds the scan"; specification 13, "Continue" hold duration and "Pause").
//!
//! A stop request is independent of a hold, so a breakpoint, a pause or a step never delays it: the
//! scan the hold suspended runs to its end with no further pause, step or breakpoint, the resource
//! takes its ordinary stop path, and the debugger cannot hold that resource's scan again. The tests
//! run the scan on the resource thread, as the runtime does, and observe it from the test thread;
//! `Held::ended_by_stop` releases the hold by hand only when the stop request alone failed to, so a
//! failed assertion never leaves a scan waiting.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as WallDuration, Instant};

use trust_runtime::debug::{DebugBreakpoint, DebugControl, DebugStop, DebugStopReason};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoDriver, IoSafeState};
use trust_runtime::retain::RetainStore;
use trust_runtime::scheduler::{ResourceHandle, ResourceRunner, ResourceState, StdClock};
use trust_runtime::value::{Duration, Value};
use trust_runtime::{RestartMode, RetainSnapshot, Runtime};

/// The longest a fixture waits for a stop event, so a missing hold fails instead of blocking.
const WAIT: WallDuration = WallDuration::from_secs(5);
/// A stop must end a held resource within this; a step, pause or breakpoint that delayed it would
/// exceed it.
const STOP_DEADLINE: WallDuration = WallDuration::from_secs(1);
/// Long enough that a hold without a stop clearly has time to end if it were going to.
const HOLD_DWELL: WallDuration = WallDuration::from_millis(150);

/// `kept` is retained, so the checkpoint the stop saves shows whether the suspended scan ran to its
/// end. `lamp` is the output that the stop policy sets to FALSE after the scan has set it TRUE.
const SOURCE: &str = r#"
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

/// Waits until `condition` holds or `deadline` passes, and returns whether it held.
fn wait_until(deadline: Instant, mut condition: impl FnMut() -> bool) -> bool {
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(WallDuration::from_millis(2));
    }
    condition()
}

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

/// What the tests read after the resource has ended.
struct Probes {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    saved: Arc<Mutex<Vec<RetainSnapshot>>>,
}

impl Probes {
    /// Every output image the driver received, oldest first.
    fn images(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("driver writes lock").clone()
    }

    /// The last image the driver received, if any.
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

/// The runtime of the tests: a debug control, a recording output driver, the stop policy
/// `lamp := FALSE`, a recording retain store and the retained `kept`.
fn fixture() -> (Runtime, DebugControl, Probes) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let saved = Arc::new(Mutex::new(Vec::new()));
    let mut harness = TestHarness::from_source(SOURCE).expect("fixture compiles");
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

/// The statement boundary that the source line containing `needle` resolves to.
fn breakpoint_on(runtime: &Runtime, needle: &str) -> DebugBreakpoint {
    let line: u32 = SOURCE
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("the source has no line containing {needle}"))
        .try_into()
        .expect("the source has fewer than u32::MAX lines");
    let location = runtime
        .resolve_breakpoint_location(SOURCE, 0, line, 0)
        .unwrap_or_else(|| panic!("the source line for {needle} has no statement boundary"));
    DebugBreakpoint::new(location)
}

/// A resource whose scan the debugger holds, with the leave-behind of the hold.
struct Held {
    handle: ResourceHandle<StdClock>,
    debug: DebugControl,
    probes: Probes,
    /// The stop of the hold, or the timeout when the scan never waited.
    hold: Result<DebugStop, mpsc::RecvTimeoutError>,
    /// The stop events, kept so the channel of the hold stays open.
    events: mpsc::Receiver<DebugStop>,
}

impl Held {
    /// Waits until the resource has ended `Stopped` within the stop deadline and joins its thread.
    /// When the stop request alone did not end it, the hold is released by hand so that no test
    /// leaves a scan waiting. Returns whether the stop request alone ended the resource.
    fn ended_by_stop(&mut self) -> bool {
        let ended = wait_until(Instant::now() + STOP_DEADLINE, || {
            self.handle.state() == ResourceState::Stopped
        });
        if !ended {
            self.debug.continue_run();
        }
        self.handle.join().expect("the resource thread ends");
        ended
    }
}

/// Spawns the resource with `debug`, then returns once the hold's stop has arrived.
fn start(runtime: Runtime, debug: DebugControl, probes: Probes, name: &str) -> Held {
    let (stop_tx, events) = mpsc::channel();
    debug.set_stop_sender(stop_tx);
    let handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
        .spawn(name)
        .expect("spawn the resource");
    let hold = events.recv_timeout(WAIT);
    Held {
        handle,
        debug,
        probes,
        hold,
        events,
    }
}

/// A resource held at a debug-mode pause requested before it starts.
fn spawn_paused(name: &str) -> Held {
    let (runtime, debug, probes) = fixture();
    debug.pause();
    start(runtime, debug, probes, name)
}

/// A resource held at the first of `needles`' statement boundaries.
fn spawn_at_breakpoints(name: &str, needles: &[&str]) -> Held {
    let (runtime, debug, probes) = fixture();
    let breakpoints = needles
        .iter()
        .map(|needle| breakpoint_on(&runtime, needle))
        .collect();
    debug.set_breakpoints_for_file(0, breakpoints);
    start(runtime, debug, probes, name)
}

/// Spec 11 section 6.9 and spec 13 "Pause": a stop ends a breakpoint hold without a resume. The
/// suspended scan runs its later statements once (`kept` 7 -> 8), the stop output `lamp := FALSE` is
/// the last image the driver receives, and the resource ends `Stopped` within one second.
#[test]
fn a_stop_ends_a_breakpoint_hold_and_the_scan_finishes() {
    let mut held = spawn_at_breakpoints("stop-breakpoint-hold", &["kept := kept + DINT#1;"]);
    let held_by_debugger = held.debug.is_paused();

    held.handle.stop();
    let ended = held.ended_by_stop();

    let stop = held
        .hold
        .as_ref()
        .expect("the scan waits at its breakpoint");
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);
    assert!(
        held_by_debugger,
        "the breakpoint holds the scan when the stop is requested"
    );
    assert!(
        ended,
        "a stop request ends a resource held at a breakpoint, without a resume"
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
        "the suspended scan ran both statements exactly once"
    );
}

/// Spec 11 section 6.9 and spec 13 "Continue" hold duration: a stop is not delayed by a step. From
/// the breakpoint hold the step is armed and the stop follows it; the scan runs to its end (`kept`
/// 7 -> 8) and the resource ends `Stopped` within one second instead of waiting for a `resume` at
/// the step stop.
#[test]
fn a_stop_during_a_step_is_not_delayed_by_the_step() {
    let mut held = spawn_at_breakpoints("stop-during-step", &["kept := kept + DINT#1;"]);
    assert_eq!(
        held.hold
            .as_ref()
            .expect("the scan waits at its breakpoint")
            .reason,
        DebugStopReason::Breakpoint
    );
    assert!(held.debug.is_paused(), "the breakpoint holds the scan");

    held.debug.step();
    held.handle.stop();
    let ended = held.ended_by_stop();

    assert!(ended, "the step does not delay the stop");
    assert_eq!(held.handle.state(), ResourceState::Stopped);
    assert_eq!(held.handle.last_error(), None);
    assert_eq!(
        held.probes.last_kept(),
        Some(Value::DInt(8)),
        "the suspended scan ran both statements exactly once"
    );
    assert_eq!(held.probes.last_image(), Some(vec![0u8]));
}

/// Spec 11 section 6.9: after the stop request the debugger cannot hold that resource's scan again,
/// so the breakpoint on the second statement is ignored for the rest of the scan. No second
/// breakpoint stop is emitted, the scan runs past it to its end (`kept` 7 -> 8) and the resource
/// ends `Stopped`.
#[test]
fn after_a_stop_no_breakpoint_holds_the_scan_again() {
    let mut held = spawn_at_breakpoints(
        "stop-breakpoint-again",
        &["kept := kept + DINT#1;", "lamp := TRUE;"],
    );
    assert_eq!(
        held.hold
            .as_ref()
            .expect("the scan waits at its breakpoint")
            .reason,
        DebugStopReason::Breakpoint
    );

    held.handle.stop();
    let ended = held.ended_by_stop();
    let further = held.events.try_recv();

    assert!(
        ended,
        "after a stop the second breakpoint does not hold the scan again"
    );
    assert_eq!(held.handle.state(), ResourceState::Stopped);
    assert_eq!(held.handle.last_error(), None);
    assert_eq!(
        held.probes.last_kept(),
        Some(Value::DInt(8)),
        "the scan ran past the second breakpoint to its end"
    );
    assert_eq!(held.probes.last_image(), Some(vec![0u8]));
    assert!(
        further.is_err(),
        "no second breakpoint stop is emitted after the stop request"
    );
}

/// Spec 11 section 6.9 and spec 13 "Continue": a hold without a stop still waits. While the
/// debugger holds the scan it commits no output; `resume` releases it and the scan commits its
/// output (`lamp := TRUE` -> image 1), so the hold waited for the resume and nothing else.
#[test]
fn a_hold_without_a_stop_still_waits_for_resume() {
    let mut held = spawn_paused("hold-without-stop");
    let stop = held
        .hold
        .as_ref()
        .expect("the scan waits at a statement boundary");
    assert_eq!(stop.reason, DebugStopReason::Pause);
    assert!(held.debug.is_paused(), "the debugger holds the scan");

    thread::sleep(HOLD_DWELL);
    assert_eq!(
        held.probes.images(),
        Vec::<Vec<u8>>::new(),
        "a hold without a stop commits no output"
    );
    assert_eq!(
        held.handle.state(),
        ResourceState::Running,
        "a hold is not a resource state"
    );
    assert_eq!(held.handle.last_error(), None);
    assert!(
        held.debug.is_paused(),
        "the debugger still holds the scan after the dwell"
    );

    held.debug.continue_run();
    let committed = wait_until(Instant::now() + WAIT, || !held.probes.images().is_empty());
    held.handle.stop();
    held.handle.join().expect("the resource thread ends");

    assert!(
        committed,
        "resume releases the hold and the scan commits its output"
    );
    assert_eq!(
        held.probes.images().first().cloned(),
        Some(vec![1u8]),
        "the resumed scan set lamp TRUE"
    );
    assert_eq!(held.handle.last_error(), None);
}

/// Spec 11 section 6.9 and spec 34 STATE-08: `shutdown` stops the program and not the scan thread,
/// so a `restart` starts the program again on the same debugger, and the debugger holds its scan
/// again. The stop released the debugger, which keeps it from holding the scan until the program
/// starts again; the breakpoint on the first statement stops the restarted program's first scan.
/// Wrong behaviour: the debugger stays released after the stop, so the restarted program runs past
/// its breakpoint and no second stop arrives.
#[test]
fn a_program_started_again_after_a_stop_is_held_by_the_debugger_again() {
    let (runtime, debug, _probes) = fixture();
    let breakpoint = breakpoint_on(&runtime, "kept := kept + DINT#1;");
    debug.set_breakpoints_for_file(0, vec![breakpoint]);
    let (stop_tx, events) = mpsc::channel();
    debug.set_stop_sender(stop_tx);
    let restart: Arc<Mutex<Option<RestartMode>>> = Arc::new(Mutex::new(None));
    let mut handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
        .with_restart_signal(Arc::clone(&restart))
        .spawn("stop-then-hold-again")
        .expect("spawn the resource");
    let first = events.recv_timeout(WAIT);

    handle.control().stop_program();
    let stopped = wait_until(Instant::now() + WAIT, || {
        handle.state() == ResourceState::Stopped
    });
    *restart.lock().expect("restart signal") = Some(RestartMode::Cold);
    let second = events.recv_timeout(WAIT);
    // Release whatever holds, so that a failed assertion never leaves a scan waiting.
    handle.stop();
    debug.continue_run();
    handle.join().expect("the resource thread ends");

    assert_eq!(
        first
            .expect("the first scan stops at the breakpoint")
            .reason,
        DebugStopReason::Breakpoint
    );
    assert!(stopped, "the program stops without a resume");
    assert_eq!(
        second
            .expect("the restarted program's scan stops at the breakpoint again")
            .reason,
        DebugStopReason::Breakpoint
    );
}
