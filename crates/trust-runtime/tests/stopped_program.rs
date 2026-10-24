//! The control `shutdown` stops the program and not the scan thread (specification 34 STATE-08;
//! specification 11 sections 6.6 and 6.9). The thread keeps serving commands in the Stopped state,
//! a `restart` starts the installed program again, and only an exit request ends the thread. Every
//! wait is bounded, so a broken interleaving fails the test instead of hanging it.

mod concurrency_support;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration as WallDuration;

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::IoDriver;
use trust_runtime::scheduler::{
    ProgramLoader, ResourceCommand, ResourceHandle, ResourceRunner, ResourceState, StartedProgram,
    StdClock,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::HangAction;
use trust_runtime::{RestartMode, RuntimeBuilder};

use concurrency_support::{dint, snapshot, stop_and_join, wait_until, WAIT};

/// Counts its scans in `scans`, and keeps `marker` for the test to write and to find restored.
const COUNTER: &str = r#"
VAR_GLOBAL
    scans : DINT := DINT#0;
    marker : DINT := DINT#7;
END_VAR

PROGRAM Main
    scans := scans + DINT#1;
END_PROGRAM
"#;

/// The program that a restart installs: it starts its count at 1000.
const INSTALLED: &str = r#"
VAR_GLOBAL
    scans : DINT := DINT#1000;
    marker : DINT := DINT#7;
END_VAR

PROGRAM Main
    scans := scans + DINT#1;
END_PROGRAM
"#;

const DIVIDE: &str = r#"
VAR_GLOBAL
    divisor : DINT := DINT#1;
    quotient : DINT := DINT#0;
END_VAR

PROGRAM Main
    quotient := DINT#100 / divisor;
END_PROGRAM
"#;

/// How long a state must hold for a test to call it settled. The scan interval is 1 ms.
const HOLD: WallDuration = WallDuration::from_millis(60);

type RestartSignal = Arc<Mutex<Option<RestartMode>>>;

fn spawn_runner(source: &str, name: &str) -> (ResourceHandle<StdClock>, RestartSignal) {
    let signal: RestartSignal = Arc::new(Mutex::new(None));
    let handle = runner_for(source, &signal)
        .spawn(name)
        .expect("spawn the resource");
    (handle, signal)
}

fn runner_for(source: &str, signal: &RestartSignal) -> ResourceRunner<StdClock> {
    let harness = TestHarness::from_source(source).expect("the test program compiles");
    ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        Duration::from_millis(1),
    )
    .with_restart_signal(Arc::clone(signal))
}

fn request_restart(signal: &RestartSignal, mode: RestartMode) {
    *signal.lock().expect("restart signal") = Some(mode);
}

fn write_marker(handle: &ResourceHandle<StdClock>, value: i32) {
    let mut updates = IndexMap::new();
    updates.insert(SmolStr::new("marker"), Value::DInt(value));
    handle
        .control()
        .send_command(ResourceCommand::MeshApply {
            updates,
            source: None,
            sequence: None,
        })
        .expect("command channel open");
}

/// A loader that installs `source`, as a restart of a stopped program does, and records the
/// restart modes it was asked for.
fn installing_loader(source: &'static str, modes: Arc<Mutex<Vec<RestartMode>>>) -> ProgramLoader {
    Box::new(move |slot, mode| {
        modes.lock().expect("recorded modes").push(mode);
        let mut next = TestHarness::from_source(source)
            .expect("the installed program compiles")
            .into_runtime();
        next.restart(mode)?;
        *slot = next.into();
        Ok(StartedProgram {
            cycle_interval: Duration::from_millis(1),
            time_scale: 1,
            simulation: None,
            hang_action: HangAction::default(),
        })
    })
}

#[test]
fn a_program_stop_keeps_the_scan_thread_serving_in_stopped_and_scans_nothing() {
    // Specification 34 STATE-08: `shutdown` stops the program and the runtime stays reachable in
    // Stopped. Wrong behaviour: the thread ends with the program (the command channel closes and
    // the snapshot cannot be read), or the program keeps scanning after the stop.
    let (handle, _signal) = spawn_runner(COUNTER, "stopped-serves");
    let control = handle.control();
    wait_until("three scans", || dint(&snapshot(&control), "scans") >= 3);

    control.stop_program();
    wait_until("the stopped state", || {
        control.state() == ResourceState::Stopped
    });
    let stopped_at = dint(&snapshot(&control), "scans");
    thread::sleep(HOLD);

    assert_eq!(control.state(), ResourceState::Stopped);
    assert_eq!(control.last_error(), None, "a stop is no fault");
    assert_eq!(
        dint(&snapshot(&control), "scans"),
        stopped_at,
        "a stopped program runs no scan, and the thread still answers"
    );
    stop_and_join(handle);
}

#[test]
fn restart_after_a_program_stop_runs_the_program_again_from_its_initial_state() {
    // Specification 34 STATE-08: a cold `restart` of the stopped program starts it as a new process
    // would. The marker, written while the program runs, returns to its initial 7, which only a
    // cold start does. Wrong behaviour: the restart is ignored in Stopped, or it keeps the
    // written value.
    let (handle, signal) = spawn_runner(COUNTER, "stopped-restarts");
    let control = handle.control();
    write_marker(&handle, 99);
    wait_until("the written marker", || {
        dint(&snapshot(&control), "marker") == 99
    });

    control.stop_program();
    wait_until("the stopped state", || {
        control.state() == ResourceState::Stopped
    });
    assert_eq!(
        dint(&snapshot(&control), "marker"),
        99,
        "a stop keeps values"
    );

    request_restart(&signal, RestartMode::Cold);
    wait_until("the running state", || {
        control.state() == ResourceState::Running
    });
    wait_until("the marker back at its initial value", || {
        dint(&snapshot(&control), "marker") == 7
    });
    let first = dint(&snapshot(&control), "scans");
    wait_until("scans after the restart", || {
        dint(&snapshot(&control), "scans") > first
    });
    stop_and_join(handle);
}

#[test]
fn an_exit_request_ends_a_stopped_thread_and_leaves_the_state_stopped() {
    // Specification 11 section 6.6 and 34 STATE-08: the process, and so the thread, ends on an
    // exit request also while the program is stopped, with the resource Stopped (status 0).
    // Wrong behaviour: the stopped wait ignores the exit and the join never returns.
    let (handle, _signal) = spawn_runner(COUNTER, "stopped-exits");
    let control = handle.control();
    control.stop_program();
    wait_until("the stopped state", || {
        control.state() == ResourceState::Stopped
    });

    stop_and_join(handle);

    assert_eq!(control.state(), ResourceState::Stopped);
    assert!(
        control.send_command(ResourceCommand::Pause).is_err(),
        "the command channel closes with the thread"
    );
}

#[test]
fn a_paused_program_stops_like_a_running_one() {
    // Specification 34 STATE-08: a stop takes a paused program to Stopped, as a chip's does.
    // Wrong behaviour: the stop waits for a resume.
    let (handle, _signal) = spawn_runner(COUNTER, "stopped-from-paused");
    let control = handle.control();
    control.pause().expect("pause is queued");
    wait_until("the paused state", || {
        control.state() == ResourceState::Paused
    });

    control.stop_program();
    wait_until("the stopped state", || {
        control.state() == ResourceState::Stopped
    });
    stop_and_join(handle);
}

#[test]
fn a_program_stop_of_a_faulted_resource_stops_it_keeps_the_fault_and_a_restart_starts_it() {
    // Specification 34 STATE-08 and 11 section 6.6 (the owner, 23 October 2026): `shutdown` stops a
    // faulted resource as it stops a running one. The thread keeps serving, the resource is Stopped
    // and the root fault stays in `last_error` until the next start; a cold restart then starts the
    // program anew and clears it. Wrong behaviour: the faulted resource stays Faulted (and so takes
    // no install), the stop ends the thread, or the fault is lost before the next start.
    let (handle, signal) = spawn_runner(DIVIDE, "stopped-faulted");
    let control = handle.control();
    let mut updates = IndexMap::new();
    updates.insert(SmolStr::new("divisor"), Value::DInt(0));
    control
        .send_command(ResourceCommand::MeshApply {
            updates,
            source: None,
            sequence: None,
        })
        .expect("command channel open");
    wait_until("the divide-by-zero fault", || {
        control.state() == ResourceState::Faulted
    });

    control.stop_program();
    wait_until("the stop of the faulted resource", || {
        control.state() == ResourceState::Stopped
    });
    thread::sleep(HOLD);
    assert_eq!(control.state(), ResourceState::Stopped);
    assert_eq!(control.last_error(), Some(RuntimeError::DivisionByZero));
    let _ = snapshot(&control);

    request_restart(&signal, RestartMode::Cold);
    wait_until("the restart after the stop", || {
        control.state() == ResourceState::Running
    });
    assert_eq!(control.last_error(), None);
    stop_and_join(handle);
}

/// The first read announces itself and waits for the test to release it, like a driver call that
/// takes a long time.
struct HeldFirstRead {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    first: bool,
}

impl IoDriver for HeldFirstRead {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if self.first {
            self.first = false;
            let _ = self.entered.send(());
            let _ = self.release.recv_timeout(WAIT);
        }
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[test]
fn a_stop_drops_the_restart_queued_before_it_and_a_restart_after_it_starts_the_program() {
    // Specification 34 STATE-08: the stop is taken first at the boundary and drops the restart
    // queued before it; a restart that arrives once the state reads Stopped starts the program.
    // The scan is held inside a driver call, so both requests wait for the same boundary and
    // their order is not left to a race. Wrong behaviour: the queued restart survives the stop and
    // runs in the Stopped state.
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install the declared test runtime");
    runtime.add_io_driver(
        "held-read",
        Box::new(HeldFirstRead {
            entered: entered_tx,
            release: release_rx,
            first: true,
        }),
    );
    let signal: RestartSignal = Arc::new(Mutex::new(None));
    let handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
        .with_restart_signal(Arc::clone(&signal))
        .spawn("stopped-drops-restart")
        .expect("spawn the resource");
    let control = handle.control();
    entered_rx
        .recv_timeout(WAIT)
        .expect("the first read starts");

    request_restart(&signal, RestartMode::Cold);
    control.stop_program();
    release_tx.send(()).expect("the held read is waiting");
    wait_until("the stopped state", || {
        control.state() == ResourceState::Stopped
    });
    thread::sleep(HOLD);

    assert_eq!(
        control.state(),
        ResourceState::Stopped,
        "the restart queued before the stop was dropped"
    );
    assert_eq!(*signal.lock().expect("restart signal"), None);

    request_restart(&signal, RestartMode::Cold);
    wait_until("the running state", || {
        control.state() == ResourceState::Running
    });
    stop_and_join(handle);
}

#[test]
fn a_restart_from_stopped_installs_the_program_through_the_loader() {
    // Specification 34 STATE-08: a restart that finds the resource Stopped loads the installed
    // program again. The loader gives a program whose count starts at 1000, so the counter reads
    // at least 1000 once it runs, which the stopped program (a count of a few hundred at most
    // before its stop) cannot reach. Wrong behaviour: the restart runs the old program again, or
    // never calls the loader.
    let modes = Arc::new(Mutex::new(Vec::new()));
    let signal: RestartSignal = Arc::new(Mutex::new(None));
    let handle = runner_for(COUNTER, &signal)
        .with_program_loader(installing_loader(INSTALLED, Arc::clone(&modes)))
        .spawn("stopped-loads")
        .expect("spawn the resource");
    let control = handle.control();
    wait_until("three scans", || dint(&snapshot(&control), "scans") >= 3);
    control.stop_program();
    wait_until("the stopped state", || {
        control.state() == ResourceState::Stopped
    });
    assert!(
        modes.lock().expect("recorded modes").is_empty(),
        "stopping the program loads nothing"
    );

    request_restart(&signal, RestartMode::Cold);
    wait_until("the running state", || {
        control.state() == ResourceState::Running
    });
    wait_until("the installed program's count", || {
        dint(&snapshot(&control), "scans") >= 1000
    });
    assert_eq!(*modes.lock().expect("recorded modes"), [RestartMode::Cold]);
    stop_and_join(handle);
}

#[test]
fn a_program_that_cannot_be_loaded_faults_with_the_reason_and_the_next_restart_loads_it_again() {
    // Specification 34 STATE-08: a program that cannot start leaves the resource Faulted with the
    // reason, and the next restart loads it again rather than restarting the stale one in place.
    // Wrong behaviour: the failure ends the thread, the reason is lost, or the second restart
    // runs the old program (the loader is not called again).
    let calls = Arc::new(AtomicUsize::new(0));
    let loader_calls = Arc::clone(&calls);
    let mut installing = installing_loader(INSTALLED, Arc::new(Mutex::new(Vec::new())));
    let loader: ProgramLoader = Box::new(move |slot, mode| {
        if loader_calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(RuntimeError::InvalidBundle("broken program".into()));
        }
        installing(slot, mode)
    });
    let signal: RestartSignal = Arc::new(Mutex::new(None));
    let handle = runner_for(COUNTER, &signal)
        .with_program_loader(loader)
        .spawn("stopped-load-fails")
        .expect("spawn the resource");
    let control = handle.control();
    control.stop_program();
    wait_until("the stopped state", || {
        control.state() == ResourceState::Stopped
    });

    request_restart(&signal, RestartMode::Cold);
    wait_until("the fault of the failed start", || {
        control.state() == ResourceState::Faulted
    });
    assert_eq!(
        control.last_error(),
        Some(RuntimeError::InvalidBundle("broken program".into()))
    );
    let _ = snapshot(&control);

    request_restart(&signal, RestartMode::Cold);
    wait_until("the running state", || {
        control.state() == ResourceState::Running
    });
    wait_until("the installed program's count", || {
        dint(&snapshot(&control), "scans") >= 1000
    });
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(control.last_error(), None, "a start clears the fault");
    stop_and_join(handle);
}

#[test]
fn the_logical_time_of_a_program_started_again_begins_at_zero() {
    // Specification 34 STATE-08: a restart from Stopped starts the program as a new process would,
    // so its logical time begins at zero. The program has run for 300 ms when it stops; the time
    // it reads just after the restart is far below that. Wrong behaviour: the time continues from
    // the scheduler's clock.
    let signal: RestartSignal = Arc::new(Mutex::new(None));
    let handle = runner_for(COUNTER, &signal)
        .with_program_loader(installing_loader(COUNTER, Arc::new(Mutex::new(Vec::new()))))
        .spawn("stopped-time")
        .expect("spawn the resource");
    let control = handle.control();
    wait_until("300 ms of logical time", || {
        snapshot(&control).now.as_nanos() >= 300_000_000
    });
    control.stop_program();
    wait_until("the stopped state", || {
        control.state() == ResourceState::Stopped
    });

    request_restart(&signal, RestartMode::Cold);
    wait_until("the running state", || {
        control.state() == ResourceState::Running
    });
    let restarted = snapshot(&control).now.as_nanos();
    assert!(
        restarted < 250_000_000,
        "the restarted program's time begins at zero, found {restarted} ns"
    );
    stop_and_join(handle);
}
