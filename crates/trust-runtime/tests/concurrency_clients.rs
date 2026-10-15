//! Many clients against a scanning resource (specification 11 sections 6.8.1 and 6.9.2;
//! specification 34 section 4.3.1). Writers, variable writers, forcers, a pauser and status and
//! snapshot readers run on their own threads while the resource scans on its thread.
//!
//! Every writer runs closed-loop: it queues one write, then waits until the program's recording
//! counter shows exactly that write. A counter that moves by one per write proves that each
//! accepted write was applied once, at a scan boundary, and that none was lost or repeated.

mod concurrency_support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use smol_str::SmolStr;
use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::harness::TestHarness;
use trust_runtime::io::IoAddress;
use trust_runtime::scheduler::{
    ResourceControl, ResourceHandle, ResourceRunner, ResourceState, StdClock,
};
use trust_runtime::value::{Duration, Value};

use concurrency_support::{dint, int, snapshot, stop_and_join, wait_until};

const WRITERS: usize = 8;
const VARIABLE_WRITERS: usize = 2;
const FORCERS: usize = 2;
const ROUNDS: usize = 40;
const PAUSE_ROUNDS: usize = 10;

/// Byte offset of writer `index`'s memory word. `%MWn` names the word at byte offset n (spec 10,
/// direct addresses), so consecutive writers use even offsets and their words never overlap.
fn word_offset(index: usize) -> usize {
    index * 2
}

/// One memory word per I/O writer, with a counter per writer that records each change of the word.
/// Each variable writer has a global target and a counter that records each change of it.
fn client_source() -> String {
    let mut source = String::from("VAR_GLOBAL\n    scans : DINT := DINT#0;\n");
    for index in 0..WRITERS {
        source.push_str(&format!(
            "    mw{index} AT %MW{offset} : INT := INT#0;\n    last{index} : INT := INT#0;\n    seen{index} : DINT := DINT#0;\n",
            offset = word_offset(index)
        ));
    }
    for index in 0..VARIABLE_WRITERS {
        source.push_str(&format!(
            "    q{index} : DINT := DINT#0;\n    qseen{index} : DINT := DINT#0;\n    qlast{index} : DINT := DINT#0;\n"
        ));
    }
    for index in 0..FORCERS {
        source.push_str(&format!("    f{index} : DINT := DINT#0;\n"));
    }
    source.push_str("END_VAR\n\nPROGRAM Main\n    scans := scans + DINT#1;\n");
    for index in 0..WRITERS {
        source.push_str(&format!(
            "    IF mw{index} <> last{index} THEN\n        last{index} := mw{index};\n        seen{index} := seen{index} + DINT#1;\n    END_IF;\n"
        ));
    }
    for index in 0..VARIABLE_WRITERS {
        source.push_str(&format!(
            "    IF q{index} <> qlast{index} THEN\n        qlast{index} := q{index};\n        qseen{index} := qseen{index} + DINT#1;\n    END_IF;\n"
        ));
    }
    // A forced global keeps its forced value at every scan boundary; a released one counts on.
    for index in 0..FORCERS {
        source.push_str(&format!("    f{index} := f{index} + DINT#1;\n"));
    }
    source.push_str("END_PROGRAM\n");
    source
}

fn spawn_clients() -> (ResourceHandle<StdClock>, DebugControl) {
    let source = client_source();
    let mut harness = TestHarness::from_source(&source).expect("client fixture compiles");
    let debug = harness.runtime_mut().enable_debug();
    let runner = ResourceRunner::new(
        harness.into_runtime(),
        StdClock::new(),
        Duration::from_millis(1),
    );
    let handle = runner.spawn("concurrency-clients").expect("spawn resource");
    (handle, debug)
}

/// Queues one I/O write per round for word `index` and waits until the program has recorded it.
fn io_writer(control: ResourceControl<StdClock>, debug: DebugControl, index: usize) -> usize {
    let seen = format!("seen{index}");
    let last = format!("last{index}");
    for round in 1..=ROUNDS {
        let value = (index as i64 + 1) * 1_000 + round as i64;
        let address =
            IoAddress::parse(&format!("%MW{}", word_offset(index))).expect("memory word address");
        debug
            .try_enqueue_io_write(address, Value::LInt(value))
            .expect("one pending write per writer is inside the bound");
        wait_until(&format!("io writer {index} round {round}"), || {
            let snap = snapshot(&control);
            dint(&snap, &seen) == round as i32 && i64::from(int(&snap, &last)) == value
        });
    }
    ROUNDS
}

/// Queues one variable write per round for global `q{index}` and waits until it is recorded.
fn variable_writer(control: ResourceControl<StdClock>, debug: DebugControl, index: usize) -> usize {
    let name = format!("q{index}");
    let seen = format!("qseen{index}");
    for round in 1..=ROUNDS {
        let value = (index as i32 + 1) * 100_000 + round as i32;
        debug
            .try_enqueue_global_write(name.as_str(), Value::DInt(value))
            .expect("a new target is inside the bound");
        wait_until(&format!("variable writer {index} round {round}"), || {
            let snap = snapshot(&control);
            dint(&snap, &seen) == round as i32 && dint(&snap, &name) == value
        });
    }
    ROUNDS
}

/// Forces `f{index}`, replaces the force in place, releases it, and checks each step at the scan
/// boundaries that the scan thread publishes.
fn forcer(control: ResourceControl<StdClock>, debug: DebugControl, index: usize) -> usize {
    let name = format!("f{index}");
    let target = ForcedVarTarget::Global(SmolStr::new(&name));
    for round in 0..ROUNDS {
        let forced = 50_000 + index as i32 * 10_000 + round as i32;
        debug
            .try_force_global(name.as_str(), Value::DInt(forced))
            .expect("a new force is inside the bound");
        wait_until(&format!("force {index} round {round} visible"), || {
            dint(&snapshot(&control), &name) == forced
        });
        for _ in 0..5 {
            assert_eq!(
                dint(&snapshot(&control), &name),
                forced,
                "a forced value holds at every scan boundary"
            );
        }
        let replaced = forced + 500;
        debug
            .try_force_global(name.as_str(), Value::DInt(replaced))
            .expect("forcing a forced target replaces it in place");
        wait_until(&format!("force {index} round {round} replaced"), || {
            dint(&snapshot(&control), &name) == replaced
        });
        debug.release_global(&name);
        assert_eq!(
            debug.forced_var_value(&target),
            None,
            "a release removes the force"
        );
        wait_until(&format!("force {index} round {round} released"), || {
            dint(&snapshot(&control), &name) != replaced
        });
    }
    ROUNDS
}

/// Pauses and resumes the scan in turn. Each step waits for the state it asked for.
fn pauser(control: ResourceControl<StdClock>) -> usize {
    for _ in 0..PAUSE_ROUNDS {
        control.pause().expect("pause is queued");
        wait_until("the scan pauses", || {
            control.state() == ResourceState::Paused
        });
        control.resume().expect("resume is queued");
        wait_until("the scan resumes", || {
            control.state() == ResourceState::Running
        });
    }
    PAUSE_ROUNDS
}

/// Reads the status view until the test ends. No fault or hang is injected, so none may appear.
fn status_reader(control: ResourceControl<StdClock>, done: Arc<AtomicBool>) -> usize {
    let mut reads = 0;
    while !done.load(Ordering::SeqCst) {
        let state = control.state();
        assert!(
            matches!(
                state,
                ResourceState::Boot | ResourceState::Running | ResourceState::Paused
            ),
            "no fault or stop is injected, found {state:?}"
        );
        assert!(control.last_error().is_none(), "no error is injected");
        assert!(!control.is_hung(), "no call is blocked");
        reads += 1;
    }
    reads
}

/// Reads the scan counter until the test ends. A scan counter never moves backwards.
fn scan_reader(control: ResourceControl<StdClock>, done: Arc<AtomicBool>) -> usize {
    let mut previous = 0;
    let mut reads = 0;
    while !done.load(Ordering::SeqCst) {
        let scans = dint(&snapshot(&control), "scans");
        assert!(scans >= previous, "the scan counter never moves backwards");
        previous = scans;
        reads += 1;
    }
    reads
}

#[test]
fn clients_writes_forces_and_status_calls_interleave_with_the_scan() {
    let (handle, debug) = spawn_clients();
    let control = handle.control();
    wait_until("the resource scans", || {
        control.state() == ResourceState::Running
    });

    let done = Arc::new(AtomicBool::new(false));
    let mut readers = Vec::new();
    for _ in 0..2 {
        let control = control.clone();
        let done = Arc::clone(&done);
        readers.push(thread::spawn(move || status_reader(control, done)));
    }
    {
        let control = control.clone();
        let done = Arc::clone(&done);
        readers.push(thread::spawn(move || scan_reader(control, done)));
    }

    let mut writers = Vec::new();
    for index in 0..WRITERS {
        let control = control.clone();
        let debug = debug.clone();
        writers.push(thread::spawn(move || io_writer(control, debug, index)));
    }
    let mut variable_writers = Vec::new();
    for index in 0..VARIABLE_WRITERS {
        let control = control.clone();
        let debug = debug.clone();
        variable_writers.push(thread::spawn(move || {
            variable_writer(control, debug, index)
        }));
    }
    let mut forcers = Vec::new();
    for index in 0..FORCERS {
        let control = control.clone();
        let debug = debug.clone();
        forcers.push(thread::spawn(move || forcer(control, debug, index)));
    }
    let pauser_control = control.clone();
    let pauser_thread = thread::spawn(move || pauser(pauser_control));

    for writer in writers {
        assert_eq!(writer.join().expect("io writer thread"), ROUNDS);
    }
    for writer in variable_writers {
        assert_eq!(writer.join().expect("variable writer thread"), ROUNDS);
    }
    for forcer in forcers {
        assert_eq!(forcer.join().expect("forcer thread"), ROUNDS);
    }
    assert_eq!(pauser_thread.join().expect("pauser thread"), PAUSE_ROUNDS);

    done.store(true, Ordering::SeqCst);
    for reader in readers {
        assert!(
            reader.join().expect("reader thread") > 0,
            "every reader completed reads"
        );
    }

    // Every accepted I/O write was applied and consumed; none is left queued or repeated.
    assert!(
        debug.drain_io_writes().is_empty(),
        "no accepted I/O write is left behind"
    );
    let snap = snapshot(&control);
    for index in 0..WRITERS {
        assert_eq!(
            dint(&snap, &format!("seen{index}")),
            ROUNDS as i32,
            "I/O writer {index}: each accepted write was applied exactly once"
        );
        assert_eq!(
            i64::from(int(&snap, &format!("last{index}"))),
            (index as i64 + 1) * 1_000 + ROUNDS as i64,
            "I/O writer {index}: the last accepted value holds"
        );
    }
    for index in 0..VARIABLE_WRITERS {
        assert_eq!(
            dint(&snap, &format!("qseen{index}")),
            ROUNDS as i32,
            "variable writer {index}: each accepted write was applied exactly once"
        );
    }
    for index in 0..FORCERS {
        let target = ForcedVarTarget::Global(SmolStr::new(format!("f{index}")));
        assert_eq!(
            debug.forced_var_value(&target),
            None,
            "forcer {index} released its last force"
        );
    }
    stop_and_join(handle);
}
