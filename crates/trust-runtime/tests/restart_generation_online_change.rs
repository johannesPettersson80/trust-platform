//! An online change of a running program (spec 11 §6.7.1, spec 34 §11.3): the commit clears the
//! queued writes and forces of the replaced lifecycle, the host's drivers keep serving the new
//! program, and engineering aliases keep working through restarts and online changes.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use trust_runtime::debug::DebugControl;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoDriver};
use trust_runtime::value::Value;

/// A harness running the compiled program of `source`, after a cold start.
fn installed(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    harness
}

#[derive(Debug)]
struct CountingIoDriver {
    reads: Arc<AtomicUsize>,
    writes: Arc<AtomicUsize>,
}

impl IoDriver for CountingIoDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Spec 11 §6.7.1 and §6.9.2: a successful online-change commit clears queued debug writes and active forces. Neither the forced value nor the queued write applies in the replacement lifecycle.
#[test]
fn successful_online_change_clears_queued_writes_and_active_forces() {
    let source_v1 = r#"
VAR_GLOBAL count : INT := INT#0; hold : INT := INT#0; END_VAR
PROGRAM Main
count := count + INT#1;
END_PROGRAM
"#;
    let source_v2 = r#"
VAR_GLOBAL count : INT := INT#0; hold : INT := INT#0; END_VAR
PROGRAM Main
count := count + INT#10;
END_PROGRAM
"#;
    let mut harness = installed(source_v1);
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    debug
        .try_force_global("count", Value::Int(99))
        .expect("the force queue has room");
    debug
        .try_enqueue_global_write("hold", Value::Int(7))
        .expect("the write queue has room");

    harness
        .reload_source(source_v2)
        .expect("apply online change");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected errors after reload: {:?}",
        cycle.errors
    );

    assert_eq!(
        harness.runtime().read_variable("count"),
        Ok(Value::Int(10)),
        "the replacement scan must run the new body from zero, not the cleared force"
    );
    assert_eq!(
        harness.runtime().read_variable("hold"),
        Ok(Value::Int(0)),
        "the queued write must not be replayed after the commit"
    );
}

/// Spec 34 §11.3: host driver objects survive a replacement while the candidate's PLC bindings are committed with it, so the same driver keeps serving cycles after the commit.
#[test]
fn host_io_driver_survives_online_change_and_keeps_serving_cycles() {
    let source_v1 = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := FALSE;
END_PROGRAM
"#;
    let source_v2 = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := TRUE;
END_PROGRAM
"#;
    let mut harness = installed(source_v1);
    let reads = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    harness.runtime_mut().add_io_driver(
        "counting",
        Box::new(CountingIoDriver {
            reads: Arc::clone(&reads),
            writes: Arc::clone(&writes),
        }),
    );
    harness
        .runtime_mut()
        .execute_cycle()
        .expect("cycle before replacement");
    let writes_before = writes.load(Ordering::SeqCst);
    assert!(
        writes_before >= 1,
        "the driver must receive outputs before the replacement"
    );

    harness
        .reload_source(source_v2)
        .expect("apply online change");
    harness
        .runtime_mut()
        .execute_cycle()
        .expect("cycle after replacement");

    assert!(
        writes.load(Ordering::SeqCst) > writes_before,
        "the surviving driver must receive outputs from the replacement generation"
    );
}

const ALIASED: &str = "PROGRAM Main VAR member : INT := INT#2; lamp AT %QX0.0 : BOOL; END_VAR \
    lamp := member > INT#5; END_PROGRAM \
    CONFIGURATION C PROGRAM P : Main; \
    VAR_ACCESS Engineering : P.member : INT READ_WRITE; END_VAR \
    END_CONFIGURATION";

/// Spec 34 §11.3: a committed restart or online change keeps the engineering alias and the I/O
/// binding of the same declarations working in the new generation. A restart gives `member` its
/// initial 2 again; an online change keeps the 9 written before it (specification 11 §6.7.1).
#[test]
fn engineering_aliases_and_bindings_follow_restarts_and_online_changes() {
    let mut harness = TestHarness::from_source(ALIASED).expect("build runtime");
    let lamp = harness
        .runtime()
        .find_variable("P.lamp")
        .expect("the program's lamp");
    assert_eq!(
        harness.runtime().bound_addresses(lamp),
        [IoAddress::parse("%QX0.0").expect("an address")],
        "the fixture binds %QX0.0"
    );
    harness
        .set_access("Engineering", Value::Int(9))
        .expect("alias write");
    assert_eq!(harness.get_access("Engineering"), Some(Value::Int(9)));

    for mode in [
        trust_runtime::RestartMode::Warm,
        trust_runtime::RestartMode::Cold,
    ] {
        harness.restart(mode).expect("restart");
        assert_eq!(harness.get_access("Engineering"), Some(Value::Int(2)));
        harness
            .set_access("Engineering", Value::Int(9))
            .expect("alias write after the restart");
        assert_eq!(harness.get_access("Engineering"), Some(Value::Int(9)));
    }

    harness
        .reload_source(&ALIASED.replace("INT#5", "INT#6"))
        .expect("apply online change");
    // An online change keeps the value a restart re-initializes (specification 11 §6.7.1).
    assert_eq!(harness.get_access("Engineering"), Some(Value::Int(9)));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert!(!harness.runtime().faulted());
    // 9 > 6, so the changed program drives the lamp through the binding of the new generation.
    assert_eq!(harness.get_direct_output("%QX0.0"), Ok(Value::Bool(true)));
}
