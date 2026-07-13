use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use trust_hir::TypeId;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoDriver, IoSafeState, ModbusTcpDriver};
use trust_runtime::retain::RetainStore;
use trust_runtime::scheduler::{Clock, ResourceRunner, ResourceState, SharedGlobals};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{FaultPolicy, WatchdogAction, WatchdogPolicy};
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

#[derive(Clone, Debug)]
struct PanicOnFirstSleepClock {
    sleeps: Arc<AtomicUsize>,
}

impl PanicOnFirstSleepClock {
    fn new() -> Self {
        Self {
            sleeps: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Clock for PanicOnFirstSleepClock {
    fn now(&self) -> Duration {
        Duration::ZERO
    }

    fn sleep_until(&self, _deadline: Duration) {
        if self.sleeps.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("intentional scheduler sleep panic after running");
        }
    }

    fn wake(&self) {}
}

#[derive(Debug)]
struct RecordingDriver {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    fail_writes: bool,
}

#[derive(Debug)]
struct PanickingReadDriver;

impl IoDriver for PanickingReadDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        panic!("intentional runtime safety panic from input driver");
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[derive(Debug)]
struct PanickingRecordingDriver {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl IoDriver for PanickingRecordingDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        panic!("intentional runtime safety panic before output commit");
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.writes
            .lock()
            .expect("driver writes lock")
            .push(outputs.to_vec());
        Ok(())
    }
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

fn runtime_with_panicking_input_driver() -> Runtime {
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(1, 0, 0);
    runtime.add_io_driver("panic-input", Box::new(PanickingReadDriver));
    runtime
}

fn wait_for_fault<C>(handle: &trust_runtime::scheduler::ResourceHandle<C>)
where
    C: Clock + Clone,
{
    let deadline = Instant::now() + StdDuration::from_secs(2);
    while handle.state() != ResourceState::Faulted && Instant::now() < deadline {
        std::thread::sleep(StdDuration::from_millis(1));
    }
}

fn modbus_params(policy: &str) -> toml::Value {
    toml::from_str(&format!(
        r#"
address = "127.0.0.1:65000"
unit_id = 1
input_start = 0
output_start = 0
timeout_ms = 1
on_error = "{policy}"
"#
    ))
    .expect("modbus params")
}

#[test]
fn panic_in_io_driver_faults_resource_visibly() {
    let runtime = runtime_with_panicking_input_driver();
    let clock = StepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner.spawn("panic-driver").expect("spawn runner");

    wait_for_fault(&handle);

    let state = handle.state();
    let last_error = handle.last_error();
    let join_result = handle.join();

    assert_eq!(state, ResourceState::Faulted);
    let error = last_error.expect("panic must be surfaced as last_error");
    assert!(
        error.to_string().contains("panic"),
        "panic error should be diagnosable, got {error}"
    );
    assert!(
        join_result.is_ok(),
        "panic should be contained by scheduler"
    );
}

#[test]
fn panic_in_shared_resource_faults_without_poison_detection() {
    let mut runtime = runtime_with_panicking_input_driver();
    runtime.storage_mut().set_global("shared", Value::Int(1));
    let shared =
        SharedGlobals::from_runtime(vec!["shared".into()], &runtime).expect("shared globals");
    let clock = StepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner
        .spawn_with_shared("panic-driver-shared", shared)
        .expect("spawn shared runner");

    wait_for_fault(&handle);

    let state = handle.state();
    let last_error = handle.last_error();
    let join_result = handle.join();

    assert_eq!(state, ResourceState::Faulted);
    let error = last_error.expect("panic must be surfaced as last_error");
    assert!(
        error.to_string().contains("panic"),
        "panic error should be diagnosable, got {error}"
    );
    assert!(
        join_result.is_ok(),
        "panic should be contained by scheduler"
    );
}

#[test]
fn modbus_warn_and_ignore_policy_do_not_fault_resource_state() {
    for policy in ["warn", "ignore"] {
        let mut runtime = Runtime::new();
        runtime.io_mut().resize(2, 0, 0);
        let driver = ModbusTcpDriver::from_params(&modbus_params(policy)).expect("modbus driver");
        runtime.add_io_driver(format!("modbus-{policy}"), Box::new(driver));
        let clock = StepClock::new();
        let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
        let mut handle = runner
            .spawn(format!("modbus-{policy}-nonfatal"))
            .expect("spawn runner");

        std::thread::sleep(StdDuration::from_millis(20));

        assert_eq!(
            handle.state(),
            ResourceState::Running,
            "{policy} must keep resource running"
        );
        assert!(
            handle.last_error().is_none(),
            "{policy} must not set resource last_error"
        );
        handle.stop();
        handle.join().expect("runner join");
    }
}

#[test]
fn modbus_fault_policy_faults_resource_state() {
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(2, 0, 0);
    let driver = ModbusTcpDriver::from_params(&modbus_params("fault")).expect("modbus driver");
    runtime.add_io_driver("modbus-fault", Box::new(driver));
    let clock = StepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner.spawn("modbus-fault").expect("spawn runner");

    wait_for_fault(&handle);

    assert_eq!(handle.state(), ResourceState::Faulted);
    assert!(
        handle.last_error().is_some(),
        "fault policy must surface the driver error"
    );
    handle.join().expect("runner join");
}

#[test]
fn requested_stop_applies_safe_outputs_before_thread_exits() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = output_runtime(writes.clone(), RecordingDriver::new(writes.clone()));
    let address = IoAddress::parse("%QX0.0").expect("safe-state output address");
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((address, Value::Bool(false)));
    runtime.set_io_safe_state(safe_state);

    let clock = StepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner.spawn("stop-safe-state").expect("spawn runner");
    std::thread::sleep(StdDuration::from_millis(20));
    handle.stop();
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Stopped);
    assert!(
        handle.last_error().is_none(),
        "successful stop should not set last_error"
    );
    let writes = writes.lock().expect("driver writes lock");
    assert_eq!(
        writes.last(),
        Some(&vec![0]),
        "last physical output write must be the configured safe state"
    );
}

#[test]
fn requested_stop_retain_save_failure_is_visible() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = output_runtime(writes.clone(), RecordingDriver::new(writes.clone()));
    runtime.set_retain_store(
        Some(Box::new(FailingRetainStore)),
        Some(Duration::from_millis(0)),
    );
    runtime.mark_retain_dirty();

    let clock = StepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner.spawn("stop-retain-failure").expect("spawn runner");
    std::thread::sleep(StdDuration::from_millis(20));
    handle.stop();
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Faulted);
    let error = handle
        .last_error()
        .expect("retain save failure should be visible");
    assert!(
        error.to_string().contains("retain"),
        "expected retain save error, got {error}"
    );
}

#[test]
fn deliberate_stop_and_fault_safe_state_paths_are_explicit() {
    let stop_writes = Arc::new(Mutex::new(Vec::new()));
    let mut stop_runtime = output_runtime(
        Arc::clone(&stop_writes),
        RecordingDriver::new(Arc::clone(&stop_writes)),
    );
    let stop_address = IoAddress::parse("%QX0.0").expect("safe-state output address");
    let mut stop_safe_state = IoSafeState::default();
    stop_safe_state
        .outputs
        .push((stop_address, Value::Bool(false)));
    stop_runtime.set_io_safe_state(stop_safe_state);

    let clock = StepClock::new();
    let runner = ResourceRunner::new(stop_runtime, clock, Duration::from_millis(1));
    let mut handle = runner
        .spawn("deliberate-stop-safe-state")
        .expect("spawn runner");
    std::thread::sleep(StdDuration::from_millis(20));
    handle.stop();
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Stopped);
    assert!(handle.last_error().is_none());
    assert_eq!(
        stop_writes.lock().expect("stop writes lock").last(),
        Some(&vec![0]),
        "deliberate stop must apply configured safe output"
    );

    let fault_writes = Arc::new(Mutex::new(Vec::new()));
    let mut fault_runtime = output_runtime(
        Arc::clone(&fault_writes),
        RecordingDriver::new(Arc::clone(&fault_writes)),
    );
    fault_runtime.clear_io_drivers();
    fault_runtime.add_io_driver(
        "panic-recorder",
        Box::new(PanickingRecordingDriver {
            writes: Arc::clone(&fault_writes),
        }),
    );
    let fault_address = IoAddress::parse("%QX0.0").expect("safe-state output address");
    let mut fault_safe_state = IoSafeState::default();
    fault_safe_state
        .outputs
        .push((fault_address, Value::Bool(false)));
    fault_runtime.set_io_safe_state(fault_safe_state);

    let clock = StepClock::new();
    let runner = ResourceRunner::new(fault_runtime, clock, Duration::from_millis(1));
    let mut handle = runner.spawn("fault-safe-state").expect("spawn runner");

    wait_for_fault(&handle);
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Faulted);
    assert!(matches!(
        handle.last_error(),
        Some(RuntimeError::ResourcePanic(_))
    ));
    assert_eq!(
        fault_writes.lock().expect("fault writes lock").last(),
        Some(&vec![0]),
        "fault path must apply configured safe output through the fault policy"
    );
}

#[test]
fn persistent_fault_restart_policy_escalates_to_visible_fault() {
    let reads = Arc::new(AtomicUsize::new(0));
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(1, 0, 0);
    runtime.set_fault_policy(FaultPolicy::Restart);
    runtime.add_io_driver(
        "persistent-failure",
        Box::new(PersistentFailingReadDriver {
            reads: Arc::clone(&reads),
        }),
    );

    let clock = StepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner
        .spawn("persistent-restart-failure")
        .expect("spawn runner");

    let deadline = Instant::now() + StdDuration::from_millis(250);
    while handle.state() != ResourceState::Faulted && Instant::now() < deadline {
        std::thread::yield_now();
    }
    if handle.state() != ResourceState::Faulted {
        handle.stop();
        handle.join().expect("runner join after timeout");
        panic!(
            "persistent restart failure did not escalate to Faulted; reads={}",
            reads.load(Ordering::SeqCst)
        );
    }

    let error = handle
        .last_error()
        .expect("restart escalation should set last_error");
    assert!(
        error.to_string().contains("restart"),
        "restart escalation should be diagnosable, got {error}"
    );
    assert!(
        reads.load(Ordering::SeqCst) <= 4,
        "restart attempts should be bounded, reads={}",
        reads.load(Ordering::SeqCst)
    );
    handle.join().expect("runner join");
}

#[test]
fn watchdog_restart_action_escalates_to_visible_fault() {
    let source = r#"
PROGRAM Main
VAR
    counter : DINT := 0;
END_VAR
WHILE TRUE DO
    counter := counter + 1;
END_WHILE;
END_PROGRAM
"#;
    let mut runtime = TestHarness::from_source(source)
        .expect("compile watchdog restart program")
        .into_runtime();
    runtime.set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        timeout: Duration::from_nanos(1),
        action: WatchdogAction::Restart,
    });

    let clock = StepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner
        .spawn("watchdog-restart-limit")
        .expect("spawn runner");

    wait_for_fault(&handle);
    if handle.state() != ResourceState::Faulted {
        handle.stop();
        handle.join().expect("runner join after timeout");
        panic!("watchdog restart action did not escalate to Faulted");
    }

    match handle
        .last_error()
        .expect("watchdog restart escalation should set last_error")
    {
        RuntimeError::RestartLimitExceeded { attempts, reason } => {
            assert_eq!(attempts, 3);
            assert!(
                reason.contains("watchdog"),
                "restart root reason should mention watchdog, got {reason}"
            );
        }
        other => panic!("expected RestartLimitExceeded, got {other:?}"),
    }
    handle.join().expect("runner join");
}

#[test]
fn panic_applies_safe_outputs_before_faulting_resource() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = output_runtime(
        Arc::clone(&writes),
        RecordingDriver::new(Arc::clone(&writes)),
    );
    runtime.clear_io_drivers();
    runtime.add_io_driver(
        "panic-recorder",
        Box::new(PanickingRecordingDriver {
            writes: Arc::clone(&writes),
        }),
    );
    let address = IoAddress::parse("%QX0.0").expect("safe-state output address");
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((address, Value::Bool(false)));
    runtime.set_io_safe_state(safe_state);

    let clock = StepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner.spawn("panic-safe-state").expect("spawn runner");

    wait_for_fault(&handle);
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Faulted);
    let error = handle.last_error().expect("panic should set last_error");
    assert!(
        error.to_string().contains("panic"),
        "expected panic error, got {error}"
    );
    assert_eq!(
        writes.lock().expect("driver writes lock").last(),
        Some(&vec![0]),
        "panic fault policy must apply configured safe output"
    );
}

#[test]
fn panic_does_not_follow_ordinary_restart_policy() {
    let mut runtime = runtime_with_panicking_input_driver();
    runtime.set_fault_policy(FaultPolicy::Restart);
    let clock = StepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner
        .spawn("panic-no-restart-policy")
        .expect("spawn runner");

    wait_for_fault(&handle);
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Faulted);
    let error = handle.last_error().expect("panic should set last_error");
    assert!(
        matches!(error, RuntimeError::ResourcePanic(_)),
        "panic should remain the visible fault, got {error}"
    );
}

#[test]
fn unexpected_thread_unwind_after_running_faults_control_surface() {
    let runtime = Runtime::new();
    let clock = PanicOnFirstSleepClock::new();
    let runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    let mut handle = runner
        .spawn("unexpected-unwind-guard")
        .expect("spawn runner");

    wait_for_fault(&handle);
    assert_eq!(handle.state(), ResourceState::Faulted);
    let error = handle
        .last_error()
        .expect("unexpected thread unwind must set last_error");
    assert!(
        matches!(error, RuntimeError::ResourcePanic(_)),
        "unexpected thread unwind should surface as ResourcePanic, got {error:?}"
    );
    assert!(
        error.to_string().contains("scheduler sleep"),
        "panic payload should be diagnosable, got {error}"
    );
    assert!(
        handle.join().is_ok(),
        "resource thread guard should contain unexpected unwind"
    );
}

#[test]
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
fn nonfinite_typed_output_faults_before_driver_commit() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(0, 4, 0);
    runtime
        .storage_mut()
        .set_global("out", Value::Real(f32::NAN));
    runtime.io_mut().bind_typed(
        "out",
        IoAddress::parse("%QD0").expect("REAL output address"),
        TypeId::REAL,
    );
    runtime.add_io_driver(
        "recorder",
        Box::new(RecordingDriver::new(Arc::clone(&writes))),
    );

    let err = runtime
        .execute_cycle()
        .expect_err("non-finite typed output must fail the cycle");

    assert_eq!(
        err,
        RuntimeError::IoDriver("typed REAL process-image value must be finite".into())
    );
    assert!(
        writes.lock().expect("driver writes lock").is_empty(),
        "rejected output must not reach the physical driver"
    );
}

#[test]
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
