//! Fail-closed runtime safety: panics, driver and retain failures, watchdog breaches and stops end
//! in a visible fault or a confirmed safe state, never in silently wrong outputs.
//!
//! The tests that run a program need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use trust_hir::TypeId;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoDriver, IoDriverHealth, IoSafeState, ModbusTcpDriver};
use trust_runtime::retain::RetainStore;
use trust_runtime::scheduler::{Clock, ResourceRunner, ResourceState};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{FaultPolicy, WatchdogAction, WatchdogPolicy};
use trust_runtime::{CompiledRuntime, RetainSnapshot};

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
struct DegradedWriteDriver {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl IoDriver for DegradedWriteDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.writes
            .lock()
            .expect("degraded driver writes lock")
            .push(outputs.to_vec());
        Ok(())
    }

    fn health(&self) -> IoDriverHealth {
        IoDriverHealth::Degraded {
            error: "safe-state handoff pending".into(),
        }
    }
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

/// The compiled program of `source`, loaded.
fn compiled(source: &str) -> CompiledRuntime {
    TestHarness::from_source(source)
        .expect("install declared test runtime")
        .into_runtime()
}

/// A TRUE output at `%QX0.0`; the program does nothing else.
const OUTPUT: &str = "VAR_GLOBAL out AT %QX0.0 : BOOL := TRUE; END_VAR PROGRAM Main END_PROGRAM";
/// [`OUTPUT`] with a retained variable, so that the retain file has a value to save.
const OUTPUT_AND_RETAINED: &str = "VAR_GLOBAL out AT %QX0.0 : BOOL := TRUE; END_VAR \
    VAR_GLOBAL RETAIN kept : INT := 1; END_VAR PROGRAM Main END_PROGRAM";
/// One input bit, so the input image has the byte a driver reads.
const ONE_INPUT: &str = "VAR_GLOBAL sensor AT %IX0.0 : BOOL; END_VAR PROGRAM Main END_PROGRAM";

fn output_runtime_of(
    source: &str,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    driver: RecordingDriver,
) -> CompiledRuntime {
    let mut runtime = compiled(source);
    runtime.add_io_driver("recorder", Box::new(driver));
    assert!(writes.lock().expect("driver writes lock").is_empty());
    runtime
}

fn output_runtime(writes: Arc<Mutex<Vec<Vec<u8>>>>, driver: RecordingDriver) -> CompiledRuntime {
    output_runtime_of(OUTPUT, writes, driver)
}

fn runtime_with_panicking_input_driver() -> CompiledRuntime {
    let mut runtime = compiled(ONE_INPUT);
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

/// One input word: an input image of two bytes.
const TWO_INPUT_BYTES: &str = "VAR_GLOBAL word_in AT %IW0 : WORD; END_VAR PROGRAM Main END_PROGRAM";

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
    handle.stop();
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
        // One input word, so the input image has the two bytes the driver reads.
        let mut runtime = compiled(TWO_INPUT_BYTES);
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
    let mut runtime = compiled(TWO_INPUT_BYTES);
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
    handle.stop();
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
fn requested_stop_faults_when_safe_state_handoff_is_unconfirmed() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    // One output bit, so the output image has the byte the safe state writes.
    let mut runtime =
        compiled("VAR_GLOBAL lamp AT %QX0.0 : BOOL; END_VAR PROGRAM Main END_PROGRAM");
    let address = IoAddress::parse("%QX0.0").expect("safe-state output address");
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((address, Value::Bool(false)));
    runtime.set_io_safe_state(safe_state);
    runtime.add_io_driver(
        "degraded-safe-state",
        Box::new(DegradedWriteDriver {
            writes: Arc::clone(&writes),
        }),
    );

    let runner = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(1));
    let mut handle = runner
        .spawn("unconfirmed-stop-safe-state")
        .expect("spawn runner");
    std::thread::sleep(StdDuration::from_millis(20));
    handle.stop();
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Faulted);
    let error = handle
        .last_error()
        .expect("unconfirmed safe-state handoff must set last_error");
    assert!(
        error.to_string().contains("degraded-safe-state")
            && error.to_string().contains("unconfirmed"),
        "expected named unconfirmed safe-state error, got {error}"
    );
    assert_eq!(
        writes.lock().expect("degraded driver writes lock").last(),
        Some(&vec![0]),
        "the driver must still receive the configured safe output attempt"
    );
}

#[test]
fn requested_stop_retain_save_failure_is_visible() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = output_runtime_of(
        OUTPUT_AND_RETAINED,
        writes.clone(),
        RecordingDriver::new(writes.clone()),
    );
    runtime
        .set_retain_store(
            Some(Box::new(FailingRetainStore)),
            Some(Duration::from_millis(0)),
        )
        .expect("the retain file is set");

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
    handle.stop();
    handle.join().expect("runner join");

    // Specification 34 STATE-08 (0.55; the owner, 23 October 2026): a stop leaves a Faulted
    // resource Stopped with its fault reported until the next start.
    assert_eq!(handle.state(), ResourceState::Stopped);
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
    let mut runtime = compiled(ONE_INPUT);
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
    handle.stop();
    handle.join().expect("runner join");
}

#[test]
fn watchdog_restart_action_escalates_to_visible_fault() {
    // Every scan overruns the watchdog's 1 ns, so its Restart action restarts the program until the
    // restart limit escalates to a visible fault. An endless loop would end at the statement
    // budget first (specification 44 NX-10), which is not the watchdog (NX-11).
    let source = r#"
PROGRAM Main
VAR
    counter : DINT := 0;
END_VAR
counter := counter + 1;
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
    handle.stop();
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
    handle.stop();
    handle.join().expect("runner join");

    // Specification 34 STATE-08 (0.55; the owner, 23 October 2026): a stop leaves a Faulted
    // resource Stopped with its fault reported until the next start.
    assert_eq!(handle.state(), ResourceState::Stopped);
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
    handle.stop();
    handle.join().expect("runner join");

    // Specification 34 STATE-08 (0.55; the owner, 23 October 2026): a stop leaves a Faulted
    // resource Stopped with its fault reported until the next start.
    assert_eq!(handle.state(), ResourceState::Stopped);
    let error = handle.last_error().expect("panic should set last_error");
    assert!(
        matches!(error, RuntimeError::ResourcePanic(_)),
        "panic should remain the visible fault, got {error}"
    );
}

#[test]
fn unexpected_thread_unwind_after_running_faults_control_surface() {
    let runtime = compiled("PROGRAM Main END_PROGRAM");
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
    runner.program_mut().set_watchdog_policy(WatchdogPolicy {
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
    handle.stop();
    handle.join().expect("runner join");
}

#[test]
fn watchdog_partial_safe_state_preserves_unconfigured_committed_outputs() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = compiled(
        "VAR_GLOBAL safe AT %QX0.0 : BOOL := TRUE; unconfigured AT %QX0.1 : BOOL := FALSE; \
         END_VAR PROGRAM Main END_PROGRAM",
    );
    runtime.add_io_driver("recorder", Box::new(RecordingDriver::new(writes.clone())));

    runtime
        .execute_cycle()
        .expect("baseline output image should commit");
    assert_eq!(
        writes.lock().expect("driver writes lock").as_slice(),
        &[vec![0b0000_0001]],
        "baseline must prove the last physically committed output image"
    );

    runtime
        .write_variable("unconfigured", &Value::Bool(true))
        .expect("checked test write");
    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((
        IoAddress::parse("%QX0.0").expect("safe output address"),
        Value::Bool(false),
    ));
    runtime.set_io_safe_state(safe_state);

    let clock = StepClock::new();
    let mut runner = ResourceRunner::new(runtime, clock, Duration::from_millis(1));
    runner.program_mut().set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        timeout: Duration::from_nanos(1),
        action: WatchdogAction::Halt,
    });
    let mut handle = runner
        .spawn("watchdog-partial-safe-state")
        .expect("spawn runner");
    wait_for_fault(&handle);
    handle.stop();
    handle.join().expect("runner join");

    assert!(matches!(
        handle.last_error(),
        Some(RuntimeError::WatchdogTimeout)
    ));
    // The fault drives the safe state, and the stop of the faulted resource drives it again
    // (specification 34 STATE-08): twice the configured output at FALSE, the unconfigured one at
    // its last committed FALSE, never the TRUE the faulted scan computed.
    assert_eq!(
        writes.lock().expect("driver writes lock").as_slice(),
        &[vec![0b0000_0001], vec![0b0000_0000], vec![0b0000_0000]],
        "safe-state commit must override configured outputs while retaining the last committed value for every unconfigured output"
    );
}

#[test]
fn retain_save_failure_prevents_output_commit_when_due() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = output_runtime_of(
        OUTPUT_AND_RETAINED,
        writes.clone(),
        RecordingDriver::new(writes.clone()),
    );
    runtime
        .set_retain_store(
            Some(Box::new(FailingRetainStore)),
            Some(Duration::from_nanos(0)),
        )
        .expect("the retain file is set");

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
    // HOST-09 prevents raw corruption of an installed controller. Keep the
    // original malformed-value assertion at its actual output codec owner.
    let writes = Arc::new(Mutex::new(Vec::new()));
    let mut storage = trust_runtime::memory::VariableStorage::new();
    storage.set_global("out", Value::Real(f32::NAN));
    let mut interface = trust_runtime::io::IoInterface::new();
    interface.resize(0, 4, 0);
    interface.bind_typed(
        "out",
        IoAddress::parse("%QD0").expect("REAL output address"),
        TypeId::REAL,
    );
    let mut driver = RecordingDriver::new(Arc::clone(&writes));
    let err = interface
        .write_outputs(&storage)
        .and_then(|()| driver.write_outputs(interface.outputs()))
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
fn installed_nonfinite_live_write_is_rejected_without_side_effects() {
    let mut runtime =
        compiled("VAR_GLOBAL out AT %QD0 : REAL := 1.0; END_VAR PROGRAM Main END_PROGRAM");
    let writes = Arc::new(Mutex::new(Vec::new()));
    runtime.add_io_driver(
        "recorder",
        Box::new(RecordingDriver::new(Arc::clone(&writes))),
    );
    let address = IoAddress::parse("%QD0").unwrap();
    let outputs = runtime.read_io(&address);
    let time = runtime.current_time();
    assert!(runtime
        .write_variable("out", &Value::Real(f32::NAN))
        .is_err());
    assert_eq!(runtime.read_variable("out"), Ok(Value::Real(1.0)));
    assert_eq!(runtime.read_io(&address), outputs);
    assert_eq!(runtime.current_time(), time);
    assert!(!runtime.faulted());
    assert!(writes.lock().unwrap().is_empty());
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
    assert_eq!(runtime.last_fault(), Some(RuntimeError::WatchdogTimeout));
}
