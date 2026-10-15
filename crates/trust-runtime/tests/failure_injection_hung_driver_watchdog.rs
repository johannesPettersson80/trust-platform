//! Brief item 3 (audit A10.16, spec 34 TIME-03): an input driver whose read never
//! returns, with the runtime watchdog enabled. Spec 11, "Fault, Overrun, and
//! Watchdog Handling" (docs/specs/11-runtime-engine.md line 1302): "If the
//! watchdog timeout elapses, the runtime raises a FAULT and halts the resource."
//!
//! The driver blocks on a channel that the test releases. The release happens
//! after the observation and before every join, and the driver also gives up
//! after a bounded wait, so a broken test cannot hang the process.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriver;
use trust_runtime::scheduler::{Clock, ResourceRunner, ResourceState};
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
#[ignore = "TIME-03: hosted independent hang supervision is not implemented yet; scope H slice after round 4 (runtime-portability checklist A10)"]
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
