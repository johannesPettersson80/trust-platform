//! Faulted resource lifecycle (specification 11 section 6.6, "Faulted resource lifecycle (hosted
//! runtime)"; section 6.9.2; specification 34 STATE-06).
//!
//! A halting fault leaves the resource thread alive in `Faulted`. The thread executes no task and
//! commits no outputs, serves reads and configuration updates, and recovers only through a restart
//! or an online change. A stop request ends it, still `Faulted`.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as WallDuration, Instant};

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{SourceFile, TestHarness};
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::retain::RetainStore;
use trust_runtime::scheduler::{
    ResourceCommand, ResourceControl, ResourceHandle, ResourceRunner, ResourceState, StdClock,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime::{RestartMode, RetainSnapshot};

/// The divisor is `1` at start, so the program runs. A mesh write of `0` makes the next scan fault
/// with divide by zero. `cycles` counts scans, and `retained_value` is the only retained global.
const DIVIDE_PROGRAM: &str = r#"
VAR_GLOBAL RETAIN
    retained_value : DINT := DINT#0;
END_VAR
VAR_GLOBAL
    divisor : DINT := DINT#1;
    cycles : DINT := DINT#0;
    quotient : DINT := DINT#0;
END_VAR

PROGRAM Main
    cycles := cycles + DINT#1;
    quotient := DINT#100 / divisor;
END_PROGRAM
"#;

/// The fixed program an online change gives a faulted resource: the same globals, and a body that
/// handles the divisor 0 that faulted the running program. An online change keeps every value
/// (specification 11 §6.7.1, decision OC-1), the divisor 0 included, so the new code must handle it.
const GUARDED_PROGRAM: &str = r#"
VAR_GLOBAL RETAIN
    retained_value : DINT := DINT#0;
END_VAR
VAR_GLOBAL
    divisor : DINT := DINT#1;
    cycles : DINT := DINT#0;
    quotient : DINT := DINT#0;
END_VAR

PROGRAM Main
    cycles := cycles + DINT#1;
    IF divisor = DINT#0 THEN
        divisor := DINT#1;
    END_IF;
    quotient := DINT#100 / divisor;
END_PROGRAM
"#;

const WAIT: WallDuration = WallDuration::from_secs(5);

type RestartSignal = Arc<Mutex<Option<RestartMode>>>;

fn spawn(source: &str) -> (ResourceHandle<StdClock>, RestartSignal) {
    let harness = TestHarness::from_source(source).expect("fault fixture compiles");
    let signal: RestartSignal = Arc::new(Mutex::new(None));
    let runner = ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        Duration::from_millis(1),
    )
    .with_restart_signal(Arc::clone(&signal));
    let handle = runner.spawn("faulted-lifecycle").expect("spawn resource");
    (handle, signal)
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(WallDuration::from_millis(2));
    }
}

/// Reads one global through the scan thread's own snapshot. The reply proves the thread answers.
fn global(control: &ResourceControl<StdClock>, name: &str) -> Value {
    let (tx, rx) = mpsc::channel();
    control
        .send_command(ResourceCommand::Snapshot { respond_to: tx })
        .expect("command channel open");
    let snapshot = rx.recv_timeout(WAIT).expect("snapshot reply");
    snapshot
        .compiled
        .as_ref()
        .and_then(|state| state.value(name))
        .unwrap_or_else(|| panic!("global {name} exists"))
}

/// The program image of `source` for this computer.
fn image(source: &str) -> Vec<u8> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!(
        "trust-faulted-lifecycle-{}-{:?}",
        std::process::id(),
        thread::current().id()
    ));
    let built = build(
        &[SourceFile::new(source)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("build replacement image");
    let _ = std::fs::remove_dir_all(&work);
    built.image
}

fn inject_divide_by_zero(control: &ResourceControl<StdClock>, retained_value: Option<i32>) {
    let mut updates = IndexMap::new();
    updates.insert(SmolStr::new("divisor"), Value::DInt(0));
    if let Some(value) = retained_value {
        updates.insert(SmolStr::new("retained_value"), Value::DInt(value));
    }
    control
        .send_command(ResourceCommand::MeshApply {
            updates,
            source: None,
            sequence: None,
        })
        .expect("command channel open");
}

/// Runs the resource until it has scanned once, then makes the next scan fault.
fn fault_running_resource(
    handle: &ResourceHandle<StdClock>,
    control: &ResourceControl<StdClock>,
    retained_value: Option<i32>,
) {
    wait_until(
        "first scan",
        || matches!(global(control, "cycles"), Value::DInt(n) if n >= 1),
    );
    inject_divide_by_zero(control, retained_value);
    wait_until("divide-by-zero fault", || {
        handle.state() == ResourceState::Faulted
    });
}

fn stop_and_join(mut handle: ResourceHandle<StdClock>) {
    handle.stop();
    handle.join().expect("resource thread joins after stop");
}

#[test]
fn halting_fault_leaves_the_resource_thread_alive_in_faulted_state() {
    let (handle, _signal) = spawn(DIVIDE_PROGRAM);
    let control = handle.control();
    fault_running_resource(&handle, &control, None);

    assert_eq!(handle.last_error(), Some(RuntimeError::DivisionByZero));
    thread::sleep(WallDuration::from_millis(50));
    assert_eq!(handle.state(), ResourceState::Faulted);
    assert_eq!(handle.last_error(), Some(RuntimeError::DivisionByZero));
    assert!(!handle.is_hung(), "a fault is not a hang");
    // The reply proves that the faulted thread still serves the command channel.
    assert!(matches!(global(&control, "divisor"), Value::DInt(0)));

    stop_and_join(handle);
}

#[test]
fn warm_restart_resumes_running_and_keeps_retained_values() {
    let (handle, signal) = spawn(DIVIDE_PROGRAM);
    let control = handle.control();
    fault_running_resource(&handle, &control, Some(42));

    *signal.lock().expect("restart signal") = Some(RestartMode::Warm);
    wait_until("warm restart resumes running", || {
        handle.state() == ResourceState::Running
    });
    wait_until(
        "cycles again after restart",
        || matches!(global(&control, "cycles"), Value::DInt(n) if n >= 1),
    );

    assert_eq!(
        global(&control, "retained_value"),
        Value::DInt(42),
        "warm restart keeps retained values"
    );
    assert_eq!(
        global(&control, "divisor"),
        Value::DInt(1),
        "non-retained values are initialized again"
    );
    assert_eq!(handle.last_error(), None);
    stop_and_join(handle);
}

#[test]
fn cold_restart_resets_retained_values_and_resumes_running() {
    let (handle, signal) = spawn(DIVIDE_PROGRAM);
    let control = handle.control();
    fault_running_resource(&handle, &control, Some(42));

    *signal.lock().expect("restart signal") = Some(RestartMode::Cold);
    wait_until("cold restart resumes running", || {
        handle.state() == ResourceState::Running
    });
    wait_until(
        "cycles again after restart",
        || matches!(global(&control, "cycles"), Value::DInt(n) if n >= 1),
    );

    assert_eq!(
        global(&control, "retained_value"),
        Value::DInt(0),
        "cold restart initializes retained values"
    );
    assert_eq!(handle.last_error(), None);
    stop_and_join(handle);
}

/// A retain store whose image cannot be read, so a warm restart cannot stage its state.
struct UnreadableRetainStore;

impl RetainStore for UnreadableRetainStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Err(RuntimeError::IoTransport("retain store unreadable".into()))
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[test]
fn failing_restart_stays_faulted_with_the_new_error_until_a_restart_succeeds() {
    let mut harness = TestHarness::from_source(DIVIDE_PROGRAM).expect("fault fixture compiles");
    harness
        .runtime_mut()
        .set_retain_store(Some(Box::new(UnreadableRetainStore)), None)
        .expect("the retain store is set; it is read at the next warm restart");
    let signal: RestartSignal = Arc::new(Mutex::new(None));
    let runner = ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        Duration::from_millis(1),
    )
    .with_restart_signal(Arc::clone(&signal));
    let handle = runner.spawn("failing-restart").expect("spawn resource");
    let control = handle.control();
    fault_running_resource(&handle, &control, None);

    *signal.lock().expect("restart signal") = Some(RestartMode::Warm);
    let unreadable = RuntimeError::IoTransport("retain store unreadable".into());
    wait_until("failed warm restart records its error", || {
        handle.last_error() == Some(unreadable.clone())
    });
    assert_eq!(
        handle.state(),
        ResourceState::Faulted,
        "a failed restart stays faulted"
    );
    assert!(matches!(global(&control, "divisor"), Value::DInt(0)));

    // A cold restart does not read the store, so it recovers.
    *signal.lock().expect("restart signal") = Some(RestartMode::Cold);
    wait_until("cold restart resumes running", || {
        handle.state() == ResourceState::Running
    });
    assert_eq!(handle.last_error(), None);
    stop_and_join(handle);
}

#[test]
fn online_change_recovers_a_faulted_resource() {
    let (handle, _signal) = spawn(DIVIDE_PROGRAM);
    let control = handle.control();
    fault_running_resource(&handle, &control, None);

    // The replacement keeps the divisor 0 that faulted the program, and handles it.
    let bytes = image(GUARDED_PROGRAM);
    let (reply_tx, reply_rx) = mpsc::channel();
    control
        .send_command(ResourceCommand::ReloadBytecode {
            bytes,
            claim: trust_runtime::scheduler::ReloadClaim::default(),
            respond_to: reply_tx,
        })
        .expect("command channel open");
    let reply = reply_rx.recv_timeout(WAIT).expect("online change reply");
    if let Err(err) = reply {
        panic!("online change must apply to a faulted resource: {err}");
    }

    wait_until("online change resumes running", || {
        handle.state() == ResourceState::Running
    });
    assert_eq!(handle.last_error(), None);
    // The first scan of the new program repairs the divisor it kept, and the resource runs on.
    wait_until("the new program handles the kept divisor", || {
        matches!(global(&control, "divisor"), Value::DInt(1))
    });
    assert_eq!(handle.state(), ResourceState::Running);
    stop_and_join(handle);
}

#[test]
fn an_exit_while_faulted_stops_the_resource_and_keeps_the_fault() {
    // Specification 11 section 6.6 and 34 STATE-08: an exit request stops a faulted resource as
    // `shutdown` does and ends the thread; the resource ends Stopped with the root fault kept, so
    // the process exits with status 1. Wrong behaviour: the exit hangs in the faulted wait, or it
    // drops the fault, which would give a service manager a clean exit status.
    let (mut handle, _signal) = spawn(DIVIDE_PROGRAM);
    let control = handle.control();
    fault_running_resource(&handle, &control, None);

    handle.stop();
    let (done_tx, done_rx) = mpsc::channel();
    let joiner = thread::spawn(move || {
        let result = handle.join();
        let _ = done_tx.send(());
        result
    });
    done_rx
        .recv_timeout(WAIT)
        .expect("a stop request ends the faulted thread");
    joiner
        .join()
        .expect("joiner thread")
        .expect("resource thread ends without panicking");

    assert_eq!(control.state(), ResourceState::Stopped);
    assert_eq!(control.last_error(), Some(RuntimeError::DivisionByZero));
}

#[test]
fn mesh_writes_are_dropped_while_faulted() {
    let (handle, _signal) = spawn(DIVIDE_PROGRAM);
    let control = handle.control();
    fault_running_resource(&handle, &control, None);

    let mut updates = IndexMap::new();
    updates.insert(SmolStr::new("divisor"), Value::DInt(5));
    control
        .send_command(ResourceCommand::MeshApply {
            updates,
            source: None,
            sequence: None,
        })
        .expect("command channel open");
    let (tx, rx) = mpsc::channel();
    control
        .send_command(ResourceCommand::MeshSnapshot {
            names: vec![SmolStr::new("divisor")],
            respond_to: tx,
        })
        .expect("command channel open");
    let values = rx.recv_timeout(WAIT).expect("mesh snapshot reply");

    assert_eq!(
        values.get("divisor"),
        Some(&Value::DInt(0)),
        "fault handling is a clearing boundary, so the write is dropped"
    );
    assert_eq!(handle.state(), ResourceState::Faulted);
    stop_and_join(handle);
}

#[test]
fn pause_and_resume_do_not_leave_the_faulted_state() {
    let (handle, _signal) = spawn(DIVIDE_PROGRAM);
    let control = handle.control();
    fault_running_resource(&handle, &control, None);

    control.pause().expect("pause command is queued");
    control.resume().expect("resume command is queued");
    thread::sleep(WallDuration::from_millis(30));
    assert_eq!(handle.state(), ResourceState::Faulted);
    stop_and_join(handle);
}
