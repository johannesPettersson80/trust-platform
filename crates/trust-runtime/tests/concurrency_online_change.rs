//! Readers during a replacement, and debugger mutations across an online change (specification 11
//! sections 6.6, 6.7.1 and 6.9.2; specification 34 UPDATE-01 and UPDATE-04).
//!
//! Every generation sets `generation` and `echo` to its own number when it initializes, and each
//! scan copies `generation` into `echo`. A snapshot that mixed two generations would show
//! `echo <> generation`, and a generation that was never accepted would be out of range.

mod concurrency_support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use smol_str::SmolStr;
use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{bytecode_bytes_from_source, TestHarness};
use trust_runtime::scheduler::{
    ReloadClaim, ResourceCommand, ResourceControl, ResourceHandle, ResourceRunner, ResourceState,
    StdClock,
};
use trust_runtime::value::{Duration, Value};

use concurrency_support::{dint, snapshot, stop_and_join, wait_until, WAIT};

const GENERATIONS: i32 = 6;
/// Four bytes that no bytecode container starts with, so the decoder rejects them.
const MALFORMED: [u8; 4] = [0x00, 0x01, 0x02, 0x03];

fn generation_source(generation: i32) -> String {
    format!(
        "VAR_GLOBAL\n    generation : DINT := DINT#{generation};\n    echo : DINT := DINT#{generation};\n    cycles : DINT := DINT#0;\n    marker : DINT := DINT#0;\nEND_VAR\n\nPROGRAM Main\n    echo := generation;\n    cycles := cycles + DINT#1;\nEND_PROGRAM\n"
    )
}

fn spawn_first_generation() -> (ResourceHandle<StdClock>, DebugControl) {
    let mut harness =
        TestHarness::from_source(&generation_source(1)).expect("generation fixture compiles");
    let debug = harness.runtime_mut().enable_debug();
    let handle = ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        Duration::from_millis(1),
    )
    .spawn("online-change-readers")
    .expect("spawn resource");
    (handle, debug)
}

/// Sends one reload and waits for its reply, which the scan thread gives at a command boundary.
fn reload(control: &ResourceControl<StdClock>, bytes: Vec<u8>) -> Result<(), RuntimeError> {
    let (tx, rx) = mpsc::channel();
    control
        .send_command(ResourceCommand::ReloadBytecode {
            bytes,
            claim: ReloadClaim::default(),
            respond_to: tx,
        })
        .expect("command channel open");
    rx.recv_timeout(WAIT)
        .expect("reload reply within the bound")
        .map(|_metadata| ())
}

/// Reads snapshots until the test ends. Each snapshot must be wholly one generation, and a reader
/// never sees an older generation after a newer one.
fn snapshot_reader(control: ResourceControl<StdClock>, done: Arc<AtomicBool>) -> usize {
    let mut previous = 0;
    let mut reads = 0;
    while !done.load(Ordering::SeqCst) {
        let snap = snapshot(&control);
        let generation = dint(&snap, "generation");
        let echo = dint(&snap, "echo");
        assert_eq!(echo, generation, "a snapshot is wholly one generation");
        assert!(
            (1..=GENERATIONS).contains(&generation),
            "generation {generation} was never accepted"
        );
        assert!(
            generation >= previous,
            "a reader never sees an older generation after a newer one"
        );
        previous = generation;
        reads += 1;
    }
    reads
}

/// Reads the status view until the test ends. Neither change kind may fault or block the resource.
fn status_reader(control: ResourceControl<StdClock>, done: Arc<AtomicBool>) -> usize {
    let mut reads = 0;
    while !done.load(Ordering::SeqCst) {
        let state = control.state();
        assert!(
            matches!(state, ResourceState::Boot | ResourceState::Running),
            "neither an accepted nor a rejected change faults the resource, found {state:?}"
        );
        assert!(
            control.last_error().is_none(),
            "no change leaves an error behind"
        );
        assert!(!control.is_hung(), "no call is blocked");
        reads += 1;
    }
    reads
}

/// Alternates a rejected change with an accepted one. After each step it checks the scan
/// boundaries, so the running generation is the one that was last accepted.
fn applier(control: ResourceControl<StdClock>) -> i32 {
    let mut current = 1;
    for generation in 2..=GENERATIONS {
        let rejected = reload(&control, MALFORMED.to_vec());
        assert!(
            matches!(rejected, Err(RuntimeError::Bytecode { .. })),
            "malformed bytecode is rejected as a typed bytecode error: {rejected:?}"
        );
        let before = dint(&snapshot(&control), "cycles");
        wait_until("the running generation keeps scanning", || {
            dint(&snapshot(&control), "cycles") > before
        });
        assert_eq!(
            dint(&snapshot(&control), "generation"),
            current,
            "a rejected change leaves the running generation in place"
        );

        let bytes = bytecode_bytes_from_source(&generation_source(generation))
            .expect("replacement generation compiles");
        reload(&control, bytes).expect("an accepted generation commits");
        current = generation;
        wait_until(&format!("generation {generation} is visible"), || {
            let snap = snapshot(&control);
            dint(&snap, "generation") == generation && dint(&snap, "echo") == generation
        });
    }
    current
}

#[test]
fn readers_see_whole_generations_while_accepted_and_rejected_changes_apply() {
    let (handle, _debug) = spawn_first_generation();
    let control = handle.control();
    wait_until("generation one scans", || {
        control.state() == ResourceState::Running
    });
    // Taken before any replacement. It must keep generation one to the end.
    let first = snapshot(&control);

    let done = Arc::new(AtomicBool::new(false));
    let mut readers = Vec::new();
    for _ in 0..3 {
        let control = control.clone();
        let done = Arc::clone(&done);
        readers.push(thread::spawn(move || snapshot_reader(control, done)));
    }
    for _ in 0..2 {
        let control = control.clone();
        let done = Arc::clone(&done);
        readers.push(thread::spawn(move || status_reader(control, done)));
    }
    let applier_control = control.clone();
    let applier_thread = thread::spawn(move || applier(applier_control));

    assert_eq!(
        applier_thread.join().expect("applier thread"),
        GENERATIONS,
        "every accepted change was committed"
    );
    done.store(true, Ordering::SeqCst);
    for reader in readers {
        assert!(
            reader.join().expect("reader thread") > 0,
            "every reader completed reads"
        );
    }

    assert_eq!(
        dint(&first, "generation"),
        1,
        "an earlier snapshot keeps its generation"
    );
    assert_eq!(dint(&snapshot(&control), "generation"), GENERATIONS);
    assert_eq!(control.last_error(), None);
    assert_eq!(control.state(), ResourceState::Running);
    stop_and_join(handle);
}

#[test]
fn debugger_writes_and_forces_queued_before_a_committed_change_do_not_replay() {
    let (handle, debug) = spawn_first_generation();
    let control = handle.control();
    wait_until("generation one scans", || {
        control.state() == ResourceState::Running
    });
    control.pause().expect("pause is queued");
    wait_until("the scan pauses", || {
        control.state() == ResourceState::Paused
    });

    debug
        .try_force_global("generation", Value::DInt(999))
        .expect("a new force is inside the bound");
    debug
        .try_enqueue_global_write("marker", Value::DInt(777))
        .expect("a new target is inside the bound");
    let bytes = bytecode_bytes_from_source(&generation_source(2)).expect("generation two compiles");
    reload(&control, bytes).expect("an accepted change commits while the scan is paused");
    control.resume().expect("resume is queued");

    // A committed change clears queued writes and forces (specification 11, section 6.9.2).
    wait_until("the new generation scans", || {
        dint(&snapshot(&control), "cycles") >= 3
    });
    let snap = snapshot(&control);
    assert_eq!(
        dint(&snap, "generation"),
        2,
        "the force queued before the commit did not replay"
    );
    assert_eq!(
        dint(&snap, "marker"),
        0,
        "the write queued before the commit did not replay"
    );
    assert_eq!(
        debug.forced_var_value(&ForcedVarTarget::Global(SmolStr::new("generation"))),
        None,
        "the commit cleared the force"
    );
    stop_and_join(handle);
}

#[test]
fn queued_debugger_writes_and_forces_survive_a_rejected_change() {
    let (handle, debug) = spawn_first_generation();
    let control = handle.control();
    wait_until("generation one scans", || {
        control.state() == ResourceState::Running
    });
    control.pause().expect("pause is queued");
    wait_until("the scan pauses", || {
        control.state() == ResourceState::Paused
    });

    debug
        .try_force_global("generation", Value::DInt(999))
        .expect("a new force is inside the bound");
    debug
        .try_enqueue_global_write("marker", Value::DInt(777))
        .expect("a new target is inside the bound");
    let rejected = reload(&control, MALFORMED.to_vec());
    assert!(
        matches!(rejected, Err(RuntimeError::Bytecode { .. })),
        "malformed bytecode is rejected: {rejected:?}"
    );
    control.resume().expect("resume is queued");

    // A rejected change never reaches its commit, so the queued write and the force stay in place.
    wait_until("the queued write is applied", || {
        dint(&snapshot(&control), "marker") == 777
    });
    assert_eq!(
        dint(&snapshot(&control), "generation"),
        999,
        "the force queued before the rejection still holds"
    );
    stop_and_join(handle);
}
