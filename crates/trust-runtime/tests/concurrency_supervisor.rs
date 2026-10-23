//! Hosted hang supervision under real races (specification 11 section 6.6, "Hosted hang
//! supervision"; specification 34 TIME-03). The supervisor thread checks the scan thread's published
//! deadline on its own clock, so these tests run the supervisor at its shortest interval.
//!
//! With a 4 ms watchdog the supervisor checks every `clamp(4 ms / 4, 1 ms, 100 ms)` = 1 ms, and it
//! declares a hang only once a cycle still runs one check interval after its deadline, which is
//! 5 ms after the cycle starts. The short cycles here sleep 2 ms, so they end well inside that
//! budget.

mod concurrency_support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriver;
use trust_runtime::scheduler::{ResourceHandle, ResourceRunner, ResourceState, StdClock};
use trust_runtime::value::Duration;
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};
use trust_runtime::{RestartMode, RuntimeBuilder};

use concurrency_support::{stop_and_join, wait_until, WAIT};

/// Cycles that must complete before a short-cycle run counts as meaningful.
const CYCLES: usize = 40;

/// Every read sleeps for `pace`, so a cycle lasts about `pace` inside its watchdog budget.
struct PacedInput {
    pace: StdDuration,
    reads: Arc<AtomicUsize>,
}

impl IoDriver for PacedInput {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        thread::sleep(self.pace);
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// The first read announces itself and blocks until the test releases it, like a driver call that
/// does not return. Later reads return at once.
struct BlockingFirstRead {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    first: bool,
    reads: Arc<AtomicUsize>,
}

impl IoDriver for BlockingFirstRead {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if self.first {
            self.first = false;
            // A failed send only means that the test has stopped waiting for the announcement.
            let _ = self.entered.send(());
            match self.release.recv_timeout(WAIT) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(RuntimeError::IoTransport(
                        "the blocked read was never released".into(),
                    ));
                }
            }
        }
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// A resource whose first read blocks, with the optional restart signal wired in.
struct Blocked {
    handle: ResourceHandle<StdClock>,
    entered: mpsc::Receiver<()>,
    release: mpsc::Sender<()>,
    reads: Arc<AtomicUsize>,
    restart: Option<Arc<Mutex<Option<RestartMode>>>>,
}

fn spawn_blocked(watchdog_ms: Option<i64>, with_restart: bool) -> Blocked {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let reads = Arc::new(AtomicUsize::new(0));
    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install declared test runtime");
    runtime.add_io_driver(
        "blocking-input",
        Box::new(BlockingFirstRead {
            entered: entered_tx,
            release: release_rx,
            first: true,
            reads: Arc::clone(&reads),
        }),
    );
    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    if let Some(timeout_ms) = watchdog_ms {
        runner.runtime_mut().set_watchdog_policy(WatchdogPolicy {
            enabled: true,
            timeout: Duration::from_millis(timeout_ms),
            action: WatchdogAction::Halt,
        });
    }
    let restart = with_restart.then(|| Arc::new(Mutex::new(None)));
    if let Some(signal) = &restart {
        runner = runner.with_restart_signal(Arc::clone(signal));
    }
    let handle = runner.spawn("supervisor-races").expect("spawn resource");
    Blocked {
        handle,
        entered: entered_rx,
        release: release_tx,
        reads,
        restart,
    }
}

#[test]
fn many_short_cycles_inside_their_timeout_are_never_declared_hung() {
    let reads = Arc::new(AtomicUsize::new(0));
    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install declared test runtime");
    runtime.add_io_driver(
        "paced-input",
        Box::new(PacedInput {
            pace: StdDuration::from_millis(5),
            reads: Arc::clone(&reads),
        }),
    );
    let mut runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    // Each cycle needs 5 ms, far inside the 100 ms timeout even when a CI host oversleeps or
    // preempts the scan thread; a cycle that really overruns is a true hang, so the margin must
    // cover OS jitter. The 40 cycles together take at least 200 ms, so a supervisor that carried
    // elapsed time across cycles instead of restarting it would still declare a hang here.
    runner.runtime_mut().set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        timeout: Duration::from_millis(100),
        action: WatchdogAction::Halt,
    });
    let handle = runner.spawn("short-cycles").expect("spawn resource");
    let control = handle.control();

    wait_until("forty cycles inside their timeout", || {
        assert!(
            !control.is_hung(),
            "a cycle that finishes inside its timeout is never declared hung"
        );
        assert!(
            matches!(
                control.state(),
                ResourceState::Boot | ResourceState::Running
            ),
            "no fault is declared for a cycle inside its timeout, found {:?}",
            control.state()
        );
        reads.load(Ordering::SeqCst) >= CYCLES
    });
    assert_eq!(control.last_error(), None, "no error is recorded");
    stop_and_join(handle);
}

#[test]
fn an_overrunning_cycle_is_declared_once_and_stays_declared_until_its_call_returns() {
    let blocked = spawn_blocked(Some(4), false);
    blocked
        .entered
        .recv_timeout(WAIT)
        .expect("the first read starts");
    wait_until("the overrun is declared", || blocked.handle.is_hung());

    // A declaration neither clears nor repeats while the call is blocked, so the flag holds.
    // This is a bounded observation window for a property that must not change.
    let window_end = Instant::now() + StdDuration::from_millis(50);
    while Instant::now() < window_end {
        assert!(
            blocked.handle.is_hung(),
            "a declared hang stays declared while its call is blocked"
        );
        thread::sleep(StdDuration::from_micros(500));
    }
    assert_eq!(blocked.handle.state(), ResourceState::Faulted);
    assert_eq!(
        blocked.handle.last_error(),
        Some(RuntimeError::WatchdogTimeout)
    );

    blocked
        .release
        .send(())
        .expect("the blocked read is still waiting");
    wait_until("the blocked call returns", || !blocked.handle.is_hung());
    assert_eq!(
        blocked.handle.state(),
        ResourceState::Faulted,
        "the watchdog action keeps the resource faulted once its call returns"
    );
    assert_eq!(
        blocked.handle.last_error(),
        Some(RuntimeError::WatchdogTimeout)
    );
    stop_and_join(blocked.handle);
}

#[test]
fn a_warm_restart_after_an_overrun_is_supervised_again_without_a_false_hang() {
    // The blocked read forces the overrun whatever the timeout; 100 ms keeps the forty fast cycles
    // after the restart clear of OS scheduling jitter, which a 4 ms timeout did not.
    let blocked = spawn_blocked(Some(100), true);
    blocked
        .entered
        .recv_timeout(WAIT)
        .expect("the first read starts");
    wait_until("the overrun is declared", || blocked.handle.is_hung());
    blocked
        .release
        .send(())
        .expect("the blocked read is still waiting");
    wait_until("the blocked call returns", || !blocked.handle.is_hung());
    assert_eq!(blocked.handle.state(), ResourceState::Faulted);

    let signal = blocked
        .restart
        .as_ref()
        .expect("the restart signal is wired");
    *signal.lock().expect("restart signal lock") = Some(RestartMode::Warm);
    wait_until("the warm restart resumes the scan", || {
        blocked.handle.state() == ResourceState::Running
    });
    assert_eq!(
        blocked.handle.last_error(),
        None,
        "a successful restart clears the root fault"
    );

    let after_restart = blocked.reads.load(Ordering::SeqCst);
    wait_until("forty cycles after the restart", || {
        assert!(
            !blocked.handle.is_hung(),
            "a cycle after the restart is not declared hung"
        );
        blocked.reads.load(Ordering::SeqCst) >= after_restart + CYCLES
    });
    assert_eq!(blocked.handle.last_error(), None);
    stop_and_join(blocked.handle);
}

#[test]
fn with_the_watchdog_disabled_a_blocked_call_is_not_declared_hung() {
    let blocked = spawn_blocked(None, false);
    let control = blocked.handle.control();
    blocked
        .entered
        .recv_timeout(WAIT)
        .expect("the first read starts");

    // Without a watchdog there is no supervisor, so the blocked call is never declared a hang
    // (specification 11, section 6.6). This is a bounded observation window for that negative.
    let window_end = Instant::now() + StdDuration::from_millis(60);
    while Instant::now() < window_end {
        assert!(
            !control.is_hung(),
            "with the watchdog disabled no hang is detected"
        );
        assert_eq!(
            control.state(),
            ResourceState::Running,
            "a blocked call with no watchdog leaves the resource running"
        );
        thread::sleep(StdDuration::from_micros(500));
    }

    blocked
        .release
        .send(())
        .expect("the blocked read is still waiting");
    stop_and_join(blocked.handle);
    assert_eq!(
        control.last_error(),
        None,
        "the released call completes without a fault"
    );
}
