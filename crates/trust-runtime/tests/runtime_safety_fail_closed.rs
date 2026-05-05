use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::error::RuntimeError;
use trust_runtime::io::{IoAddress, IoDriver, IoSafeState};
use trust_runtime::retain::RetainStore;
use trust_runtime::scheduler::{Clock, ResourceRunner, ResourceState};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};
use trust_runtime::{RetainSnapshot, Runtime};

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

#[derive(Debug)]
struct RecordingDriver {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    fail_writes: bool,
}

impl RecordingDriver {
    fn new(writes: Arc<Mutex<Vec<Vec<u8>>>>) -> Self {
        Self {
            writes,
            fail_writes: false,
        }
    }

    fn failing(writes: Arc<Mutex<Vec<Vec<u8>>>>) -> Self {
        Self {
            writes,
            fail_writes: true,
        }
    }
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
        if self.fail_writes {
            return Err(RuntimeError::IoTransport(
                "safe-state output write failed".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
struct FailingRetainStore;

impl RetainStore for FailingRetainStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Ok(RetainSnapshot::default())
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        Err(RuntimeError::RetainStore("retain save failed".into()))
    }
}

fn output_runtime(writes: Arc<Mutex<Vec<Vec<u8>>>>, driver: RecordingDriver) -> Runtime {
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(0, 1, 0);
    runtime.storage_mut().set_global("out", Value::Bool(true));
    runtime
        .io_mut()
        .bind("out", IoAddress::parse("%QX0.0").expect("output address"));
    runtime.add_io_driver("recorder", Box::new(driver));
    assert!(writes.lock().expect("driver writes lock").is_empty());
    runtime
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn watchdog_deadline_breach_before_commit_prevents_output_write() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let runtime = output_runtime(writes.clone(), RecordingDriver::new(writes.clone()));
    let clock = StepClock::new();
    let mut runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    runner.runtime_mut().set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        timeout: Duration::from_nanos(1),
        action: WatchdogAction::Halt,
    });

    let mut handle = runner
        .spawn("watchdog-before-output")
        .expect("spawn runner");
    let deadline = Instant::now() + StdDuration::from_millis(250);
    while handle.state() != ResourceState::Faulted {
        assert!(
            Instant::now() < deadline,
            "resource did not fault in time; state={:?}",
            handle.state()
        );
        std::thread::yield_now();
    }

    assert!(matches!(
        handle.last_error(),
        Some(RuntimeError::WatchdogTimeout)
    ));
    assert!(
        writes.lock().expect("driver writes lock").is_empty(),
        "watchdog breach before output commit must prevent physical output writes"
    );
    handle.join().expect("runner join");
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn retain_save_failure_prevents_output_commit_when_due() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = output_runtime(writes.clone(), RecordingDriver::new(writes.clone()));
    runtime.set_retain_store(
        Some(Box::new(FailingRetainStore)),
        Some(Duration::from_nanos(0)),
    );
    runtime.mark_retain_dirty();

    let err = runtime
        .execute_cycle()
        .expect_err("due retain save failure must fail the cycle");
    assert!(
        err.to_string().contains("retain"),
        "expected retain error, got {err}"
    );
    assert!(
        writes.lock().expect("driver writes lock").is_empty(),
        "due retain save failure must prevent physical output writes"
    );
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn safe_state_write_failure_is_reported_without_losing_root_fault() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = output_runtime(writes.clone(), RecordingDriver::failing(writes.clone()));
    let address = IoAddress::parse("%QX0.0").expect("safe-state output address");
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((address, Value::Bool(false)));
    runtime.set_io_safe_state(safe_state);

    let err = runtime.watchdog_timeout();

    assert!(
        err.to_string().contains("safe-state"),
        "safe-state write failure must be reported, got {err}"
    );
    assert_eq!(runtime.last_fault(), Some(&RuntimeError::WatchdogTimeout));
}
