//! Shutdown races and callback ordering (specification 11 sections 6.6, 6.9.2 and 6.13; specification
//! 34 TIME-03). A stop is requested while commands are queued, while the resource is paused, while
//! it is faulted, and while a driver call is blocked under the hang supervisor. Each join is
//! bounded, and the final state follows specification 11.

mod concurrency_support;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoDriver};
use trust_runtime::scheduler::{
    ResourceCommand, ResourceControl, ResourceHandle, ResourceRunner, ResourceState, StdClock,
};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};
use trust_runtime::RuntimeBuilder;

use concurrency_support::{stop_and_join, wait_until, WAIT};

const QUEUED: usize = 20;

const DIVIDE_SOURCE: &str = r#"
VAR_GLOBAL
    divisor : DINT := DINT#1;
    quotient : DINT := DINT#0;
END_VAR

PROGRAM Main
    quotient := DINT#100 / divisor;
END_PROGRAM
"#;

/// The first read announces itself and blocks until the test releases it, like a driver call that
/// does not return. Later reads return at once. Output writes are counted.
struct BlockingFirstRead {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    first: bool,
    outputs: Arc<AtomicUsize>,
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
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        self.outputs.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct HungInput {
    handle: ResourceHandle<StdClock>,
    entered: mpsc::Receiver<()>,
    release: mpsc::Sender<()>,
    outputs: Arc<AtomicUsize>,
}

/// Spawns a resource whose first read blocks. The hang supervisor is enabled when `watchdog_ms`
/// is given.
fn spawn_blocking_input(name: &str, watchdog_ms: Option<i64>) -> HungInput {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let outputs = Arc::new(AtomicUsize::new(0));
    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install declared test runtime");
    runtime.add_io_driver(
        "blocking-input",
        Box::new(BlockingFirstRead {
            entered: entered_tx,
            release: release_rx,
            first: true,
            outputs: Arc::clone(&outputs),
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
    let handle = runner.spawn(name).expect("spawn resource");
    HungInput {
        handle,
        entered: entered_rx,
        release: release_tx,
        outputs,
    }
}

/// Spawns an empty resource with debugging enabled, so that its queues can be filled directly.
fn spawn_idle(name: &str) -> (ResourceHandle<StdClock>, DebugControl) {
    let mut runtime = RuntimeBuilder::new()
        .build()
        .expect("install declared test runtime");
    let debug = runtime.enable_debug();
    let handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
        .spawn(name)
        .expect("spawn resource");
    (handle, debug)
}

/// Sends snapshot commands until the channel closes. Each command is answered or dropped at
/// shutdown, and no reply is left waiting past the bound.
fn command_client(control: ResourceControl<StdClock>, served: Arc<AtomicUsize>) {
    loop {
        let (tx, rx) = mpsc::channel();
        if control
            .send_command(ResourceCommand::MeshSnapshot {
                names: Vec::new(),
                respond_to: tx,
            })
            .is_err()
        {
            return;
        }
        match rx.recv_timeout(WAIT) {
            Ok(_) => {
                served.fetch_add(1, Ordering::SeqCst);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("a command is answered or dropped within the bound")
            }
        }
    }
}

#[test]
fn stop_while_paused_ends_the_thread_in_stopped() {
    let (handle, _debug) = spawn_idle("shutdown-paused");
    let control = handle.control();
    control.pause().expect("pause is queued");
    wait_until("the scan pauses", || {
        control.state() == ResourceState::Paused
    });

    stop_and_join(handle);
    assert_eq!(
        control.state(),
        ResourceState::Stopped,
        "a stop ends a paused resource as stopped"
    );
    assert_eq!(control.last_error(), None);
}

#[test]
fn stop_during_a_hung_driver_call_drops_queued_commands_and_ends_faulted() {
    let hung = spawn_blocking_input("shutdown-hung", Some(100));
    let control = hung.handle.control();
    hung.entered
        .recv_timeout(WAIT)
        .expect("the first read starts");
    wait_until("the hang is declared", || hung.handle.is_hung());

    // The scan thread is inside the blocked call, so these commands wait in the channel.
    let mut replies = Vec::new();
    for _ in 0..QUEUED {
        let (tx, rx) = mpsc::channel();
        control
            .send_command(ResourceCommand::MeshSnapshot {
                names: Vec::new(),
                respond_to: tx,
            })
            .expect("queued while the call is blocked");
        replies.push(rx);
    }

    // The stop is requested while the call is still blocked. The release comes after it.
    hung.handle.stop();
    hung.release
        .send(())
        .expect("the blocked read is still waiting");
    let outputs = Arc::clone(&hung.outputs);
    stop_and_join(hung.handle);

    for rx in replies {
        match rx.recv_timeout(WAIT) {
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
            other => panic!("a command queued behind a stop is dropped, not answered: {other:?}"),
        }
    }
    assert_eq!(
        control.state(),
        ResourceState::Faulted,
        "the hang is the final state (specification 11, section 6.6)"
    );
    assert_eq!(control.last_error(), Some(RuntimeError::WatchdogTimeout));
    assert!(!control.is_hung(), "the blocked call has returned");
    assert_eq!(
        outputs.load(Ordering::SeqCst),
        0,
        "a cycle that overran commits no output"
    );
    assert!(
        control.send_command(ResourceCommand::Pause).is_err(),
        "the command channel closes with the thread"
    );
}

#[test]
fn stop_while_faulted_answers_or_drops_every_command_within_the_bound() {
    let harness = TestHarness::from_source(DIVIDE_SOURCE).expect("divide fixture compiles");
    let handle = ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        Duration::from_millis(1),
    )
    .spawn("shutdown-faulted")
    .expect("spawn resource");
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

    let served = Arc::new(AtomicUsize::new(0));
    let client = {
        let control = control.clone();
        let served = Arc::clone(&served);
        thread::spawn(move || command_client(control, served))
    };
    wait_until("commands are answered while faulted", || {
        served.load(Ordering::SeqCst) >= QUEUED
    });

    stop_and_join(handle);
    client.join().expect("client thread");
    assert_eq!(
        control.state(),
        ResourceState::Faulted,
        "a stop keeps a faulted resource faulted"
    );
    assert_eq!(control.last_error(), Some(RuntimeError::DivisionByZero));
}

#[test]
fn stop_clears_debug_writes_and_forces_queued_while_paused() {
    let (handle, debug) = spawn_idle("shutdown-writes");
    let control = handle.control();
    control.pause().expect("pause is queued");
    wait_until("the scan pauses", || {
        control.state() == ResourceState::Paused
    });

    let address = IoAddress::parse("%MW0").expect("memory word address");
    for number in 0..QUEUED {
        debug
            .try_enqueue_io_write(address.clone(), Value::LInt(number as i64))
            .expect("queued while paused");
    }
    debug
        .try_force_global("f0", Value::DInt(7))
        .expect("force queued while paused");

    stop_and_join(handle);
    assert!(
        debug.drain_io_writes().is_empty(),
        "a deliberate stop clears queued I/O writes (specification 11, section 6.9.2)"
    );
    assert_eq!(
        debug.forced_var_value(&ForcedVarTarget::Global(SmolStr::new("f0"))),
        None,
        "a deliberate stop clears active forces"
    );
}

/// Counts reads, and reads before the thread-start hook has run.
struct OrderedRead {
    ready: Arc<AtomicBool>,
    reads: Arc<AtomicUsize>,
    early: Arc<AtomicUsize>,
}

impl IoDriver for OrderedRead {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if !self.ready.load(Ordering::SeqCst) {
            self.early.fetch_add(1, Ordering::SeqCst);
        }
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[test]
fn thread_init_hook_runs_on_the_scan_thread_before_the_first_cycle() {
    let ready = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let early = Arc::new(AtomicUsize::new(0));
    let hook_thread: Arc<Mutex<Option<thread::ThreadId>>> = Arc::new(Mutex::new(None));
    let hook_ready = Arc::clone(&ready);
    let hook_slot = Arc::clone(&hook_thread);
    let hook: Arc<dyn Fn() -> Result<(), RuntimeError> + Send + Sync> =
        Arc::new(move || -> Result<(), RuntimeError> {
            *hook_slot.lock().expect("hook slot lock") = Some(thread::current().id());
            hook_ready.store(true, Ordering::SeqCst);
            Ok(())
        });

    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install declared test runtime");
    runtime.add_io_driver(
        "ordered-read",
        Box::new(OrderedRead {
            ready: Arc::clone(&ready),
            reads: Arc::clone(&reads),
            early: Arc::clone(&early),
        }),
    );
    let handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
        .with_thread_init_hook(hook)
        .spawn("hook-order")
        .expect("spawn resource");
    let scan_thread = handle.thread_id();

    wait_until("cycles run", || reads.load(Ordering::SeqCst) >= 5);
    stop_and_join(handle);
    assert_eq!(
        early.load(Ordering::SeqCst),
        0,
        "no cycle reads inputs before the hook returns"
    );
    assert_eq!(
        *hook_thread.lock().expect("hook thread lock"),
        Some(scan_thread),
        "the hook runs on the scan thread"
    );
}

#[test]
fn a_failing_thread_init_hook_refuses_the_start_before_any_cycle() {
    let reads = Arc::new(AtomicUsize::new(0));
    let hook: Arc<dyn Fn() -> Result<(), RuntimeError> + Send + Sync> =
        Arc::new(|| -> Result<(), RuntimeError> {
            Err(RuntimeError::ControlError("thread start refused".into()))
        });

    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install declared test runtime");
    runtime.add_io_driver(
        "ordered-read",
        Box::new(OrderedRead {
            ready: Arc::new(AtomicBool::new(false)),
            reads: Arc::clone(&reads),
            early: Arc::new(AtomicUsize::new(0)),
        }),
    );
    let runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
        .with_thread_init_hook(hook);
    match runner.spawn("hook-refused") {
        Err(RuntimeError::ControlError(text)) => {
            assert_eq!(text.as_str(), "thread start refused");
        }
        Err(other) => panic!("the hook's error is reported: {other:?}"),
        Ok(_) => panic!("a refused start must not yield a running resource"),
    }
    assert_eq!(
        reads.load(Ordering::SeqCst),
        0,
        "no cycle runs after a refused start"
    );
}

/// Logs each callback in the order that the scan thread makes it.
struct LoggingDriver {
    log: Arc<Mutex<Vec<&'static str>>>,
}

impl IoDriver for LoggingDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        self.log.lock().expect("callback log lock").push("read");
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        self.log.lock().expect("callback log lock").push("write");
        Ok(())
    }
}

#[test]
fn every_cycle_reads_inputs_before_it_publishes_outputs() {
    let log: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
    let mut builder = RuntimeBuilder::new();
    builder.io_mut().resize(1, 0, 0);
    let mut runtime = builder.build().expect("install declared test runtime");
    runtime.add_io_driver(
        "logging",
        Box::new(LoggingDriver {
            log: Arc::clone(&log),
        }),
    );
    let handle = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1))
        .spawn("callback-order")
        .expect("spawn resource");

    wait_until("six complete cycles", || {
        log.lock().expect("callback log lock").len() >= 12
    });
    // Copied before the stop, so a safe-state write at shutdown is not part of the sequence.
    let entries = log.lock().expect("callback log lock").to_vec();
    stop_and_join(handle);

    for (position, entry) in entries.iter().enumerate() {
        let expected = if position % 2 == 0 { "read" } else { "write" };
        assert_eq!(
            *entry, expected,
            "callback {position} breaks the read-then-write order of a cycle"
        );
    }
}
