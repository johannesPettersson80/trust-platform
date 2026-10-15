//! Brief item 3 (audit A10.16, spec 34 TIME-03): an input driver whose read never
//! returns, with the runtime watchdog enabled. Spec 11, "Fault, Overrun, and
//! Watchdog Handling" (docs/specs/11-runtime-engine.md line 1302): "If the
//! watchdog timeout elapses, the runtime raises a FAULT and halts the resource."
//!
//! The driver blocks on a channel that the test releases. The release happens
//! after the observation and before every join, and the driver also gives up
//! after a bounded wait, so a broken test cannot hang the process.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriver;
use trust_runtime::scheduler::{Clock, ResourceHandle, ResourceRunner, ResourceState};
use trust_runtime::value::Duration;
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};
use trust_runtime::RuntimeBuilder;

#[derive(Clone, Debug)]
struct StepClock {
    inner: Arc<Mutex<Duration>>,
}

impl StepClock {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Duration::ZERO)),
        }
    }
}

impl Clock for StepClock {
    fn now(&self) -> Duration {
        *self.inner.lock().expect("step clock lock")
    }

    fn sleep_until(&self, deadline: Duration) {
        *self.inner.lock().expect("step clock lock") = deadline;
    }

    fn wake(&self) {}
}

/// Announces that its read has started, then blocks until the test releases it.
#[derive(Debug)]
struct HungInputDriver {
    entered: Sender<()>,
    release: Receiver<()>,
}

impl IoDriver for HungInputDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        // A failed send only means the test has stopped waiting for the signal.
        let _ = self.entered.send(());
        match self.release.recv_timeout(StdDuration::from_secs(10)) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => Ok(()),
            Err(RecvTimeoutError::Timeout) => Err(RuntimeError::IoTransport(
                "hung input driver was never released".into(),
            )),
        }
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[test]
fn enabled_watchdog_faults_the_resource_while_an_input_read_is_hung() {
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();

    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install declared test runtime");
    runtime.add_io_driver(
        "hung-input",
        Box::new(HungInputDriver {
            entered: entered_tx,
            release: release_rx,
        }),
    );

    let mut runner = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(1));
    runner.runtime_mut().set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        timeout: Duration::from_millis(100),
        action: WatchdogAction::Halt,
    });
    let mut handle = runner.spawn("hung-driver-watchdog").expect("spawn runner");

    entered_rx
        .recv_timeout(StdDuration::from_secs(5))
        .expect("the input read must start before the watchdog is observed");
    // Before side: the hung cycle runs in the Running state with no latched fault.
    assert_eq!(handle.state(), ResourceState::Running);
    assert_eq!(handle.last_error(), None);

    // Expectation: the 100 ms watchdog faults the resource while the read is
    // still blocked. The wait is bounded at two seconds, far above the timeout.
    let wait_deadline = Instant::now() + StdDuration::from_secs(2);
    while handle.state() != ResourceState::Faulted && Instant::now() < wait_deadline {
        std::thread::sleep(StdDuration::from_millis(1));
    }
    let state_while_blocked = handle.state();
    let error_while_blocked = handle.last_error();

    // Release the driver only after the observation, then stop and join. The
    // stop flag ends the loop if the resource did not fault, so join returns.
    drop(release_tx);
    handle.stop();
    handle.join().expect("resource thread join after release");

    assert_eq!(
        state_while_blocked,
        ResourceState::Faulted,
        "spec 11 section 6.6: the watchdog timeout must raise a FAULT while the hung read is still blocked"
    );
    assert_eq!(
        error_while_blocked,
        Some(RuntimeError::WatchdogTimeout),
        "the watchdog fault must name the watchdog timeout"
    );
}

/// Like `HungInputDriver`, and it counts physical output writes. A test can then show that a cycle
/// the supervisor declared hung commits nothing when its blocked call finally returns.
#[derive(Debug)]
struct CountingHungDriver {
    entered: Sender<()>,
    release: Receiver<()>,
    output_writes: Arc<AtomicUsize>,
}

impl IoDriver for CountingHungDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        let _ = self.entered.send(());
        match self.release.recv_timeout(StdDuration::from_secs(10)) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => Ok(()),
            Err(RecvTimeoutError::Timeout) => Err(RuntimeError::IoTransport(
                "hung input driver was never released".into(),
            )),
        }
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        self.output_writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// A resource whose first input read blocks until the test releases it.
struct HungResource {
    handle: ResourceHandle<StepClock>,
    entered: Receiver<()>,
    release: Sender<()>,
    output_writes: Arc<AtomicUsize>,
}

fn spawn_hung_resource(timeout: Duration) -> HungResource {
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let output_writes = Arc::new(AtomicUsize::new(0));

    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install declared test runtime");
    runtime.add_io_driver(
        "hung-input",
        Box::new(CountingHungDriver {
            entered: entered_tx,
            release: release_rx,
            output_writes: Arc::clone(&output_writes),
        }),
    );

    let mut runner = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(1));
    runner.runtime_mut().set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        timeout,
        action: WatchdogAction::Halt,
    });
    let handle = runner
        .spawn("hung-driver-supervisor")
        .expect("spawn runner");
    HungResource {
        handle,
        entered: entered_rx,
        release: release_tx,
        output_writes,
    }
}

/// Releases the blocked read, stops the resource and joins it.
fn release_and_join(hung: HungResource) {
    let HungResource {
        mut handle,
        release,
        ..
    } = hung;
    let _ = release.send(());
    handle.stop();
    handle.join().expect("resource thread joins after release");
}

/// Waits for the fault that the supervisor records while the read is still blocked, and returns
/// the instant that the fault was observed.
fn wait_for_fault(handle: &ResourceHandle<StepClock>) -> Instant {
    let deadline = Instant::now() + StdDuration::from_secs(2);
    while handle.state() != ResourceState::Faulted {
        assert!(
            Instant::now() < deadline,
            "the hung read must fault the resource while it is still blocked"
        );
        std::thread::sleep(StdDuration::from_millis(1));
    }
    Instant::now()
}

#[test]
fn supervisor_records_the_hang_within_timeout_plus_two_check_intervals() {
    let hung = spawn_hung_resource(Duration::from_millis(100));
    hung.entered
        .recv_timeout(StdDuration::from_secs(5))
        .expect("the input read must start");
    let read_started = Instant::now();
    let observed = wait_for_fault(&hung.handle);

    // The deadline is armed before the read starts, so the fault is due within the timeout plus two
    // check intervals of clamp(100 ms / 4, 1 ms, 100 ms) = 25 ms. The slack covers polling and
    // scheduling on a loaded host.
    let budget = StdDuration::from_millis(100)
        + StdDuration::from_millis(2 * 25)
        + StdDuration::from_millis(100);
    let elapsed = observed.duration_since(read_started);
    assert!(
        elapsed <= budget,
        "watchdog fault recorded after {elapsed:?}, budget {budget:?}"
    );
    assert!(
        hung.handle.is_hung(),
        "the supervisor marks the scan thread hung"
    );
    assert_eq!(
        hung.handle.last_error(),
        Some(RuntimeError::WatchdogTimeout)
    );
    release_and_join(hung);
}

#[test]
fn releasing_the_blocked_read_finishes_the_scan_without_committing_outputs() {
    let hung = spawn_hung_resource(Duration::from_millis(100));
    hung.entered
        .recv_timeout(StdDuration::from_secs(5))
        .expect("the input read must start");
    wait_for_fault(&hung.handle);
    assert!(hung.handle.is_hung());
    assert_eq!(
        hung.output_writes.load(Ordering::SeqCst),
        0,
        "no output is written while the read is blocked"
    );

    hung.release
        .send(())
        .expect("the blocked read is still waiting");
    let deadline = Instant::now() + StdDuration::from_secs(5);
    while hung.handle.is_hung() {
        assert!(
            Instant::now() < deadline,
            "the scan thread must finish once its blocked read returns"
        );
        std::thread::sleep(StdDuration::from_millis(1));
    }

    assert_eq!(
        hung.handle.state(),
        ResourceState::Faulted,
        "the watchdog action keeps the resource faulted"
    );
    assert_eq!(
        hung.handle.last_error(),
        Some(RuntimeError::WatchdogTimeout)
    );
    assert_eq!(
        hung.output_writes.load(Ordering::SeqCst),
        0,
        "the hung cycle must not commit its outputs after the read returns"
    );
    let HungResource { mut handle, .. } = hung;
    handle.stop();
    handle
        .join()
        .expect("resource thread joins after the faulted wait stops");
}
