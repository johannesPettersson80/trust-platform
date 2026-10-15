use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoDriver;
use trust_runtime::scheduler::{Clock, ResourceHandle, ResourceRunner, ResourceState};
use trust_runtime::value::Duration;
use trust_runtime::watchdog::FaultPolicy;
use trust_runtime::{Runtime, RuntimeBuilder};

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
struct PersistentFailingReadDriver {
    reads: Arc<AtomicUsize>,
}

impl IoDriver for PersistentFailingReadDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Err(RuntimeError::IoTransport(
            "persistent input transport failure".into(),
        ))
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[derive(Debug)]
struct PanickingReadDriver {
    reads: Arc<AtomicUsize>,
}

impl IoDriver for PanickingReadDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        panic!("intentional restart-generation panic from input driver");
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

fn runtime_with_input_driver(policy: FaultPolicy, driver: Box<dyn IoDriver>) -> Runtime {
    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install declared test runtime");
    runtime.set_fault_policy(policy);
    runtime.add_io_driver("restart-generation-input", driver);
    runtime
}

fn wait_for_fault<C>(handle: &ResourceHandle<C>)
where
    C: Clock + Clone,
{
    let deadline = Instant::now() + StdDuration::from_secs(2);
    while handle.state() != ResourceState::Faulted && Instant::now() < deadline {
        std::thread::sleep(StdDuration::from_millis(1));
    }
}

/// Spec 11 §6.6: a persistent non-panic cycle fault under FaultPolicy::Restart escalates after three automatic restart attempts with a RestartLimitExceeded error that names the unresolved fault.
#[test]
fn persistent_cycle_fault_escalates_to_restart_limit_after_three_attempts() {
    let reads = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_input_driver(
        FaultPolicy::Restart,
        Box::new(PersistentFailingReadDriver { reads }),
    );
    let runner = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(1));
    let mut handle = runner
        .spawn("restart-limit-cycle-fault")
        .expect("spawn runner");
    wait_for_fault(&handle);
    handle.stop();
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Faulted);
    match handle
        .last_error()
        .expect("restart escalation should set last_error")
    {
        RuntimeError::RestartLimitExceeded { attempts, reason } => {
            assert_eq!(attempts, 3);
            assert!(
                reason.contains("persistent input transport failure"),
                "restart root reason should name the unresolved fault, got {reason}"
            );
        }
        other => panic!("expected RestartLimitExceeded, got {other:?}"),
    }
}

/// Spec 34 §9 STATE-06 and spec 11 §6.6: a failed automatic restart leaves the resource faulted, and no scan runs outside the bounded sequence: one failing scan, then exactly one scan after each of the three restarts.
#[test]
fn failed_restart_sequence_stays_faulted_with_exactly_one_scan_per_attempt() {
    let reads = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_input_driver(
        FaultPolicy::Restart,
        Box::new(PersistentFailingReadDriver {
            reads: Arc::clone(&reads),
        }),
    );
    let runner = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(1));
    let mut handle = runner
        .spawn("state06-restart-sequence")
        .expect("spawn runner");
    wait_for_fault(&handle);
    handle.stop();
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Faulted);
    assert_eq!(
        reads.load(Ordering::SeqCst),
        4,
        "one failing scan plus one scan after each of the three restart attempts"
    );
}

/// Spec 11 §6.6: a contained ResourcePanic is never automatically restarted; it remains the visible fault and starts no restart attempt.
#[test]
fn contained_resource_panic_is_never_retried_under_the_restart_policy() {
    let reads = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_input_driver(
        FaultPolicy::Restart,
        Box::new(PanickingReadDriver {
            reads: Arc::clone(&reads),
        }),
    );
    let runner = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(1));
    let mut handle = runner.spawn("panic-not-restarted").expect("spawn runner");
    wait_for_fault(&handle);
    handle.stop();
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Faulted);
    assert!(
        matches!(handle.last_error(), Some(RuntimeError::ResourcePanic(_))),
        "a contained panic must stay a ResourcePanic"
    );
    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "a panic must not start any restart attempt"
    );
}
