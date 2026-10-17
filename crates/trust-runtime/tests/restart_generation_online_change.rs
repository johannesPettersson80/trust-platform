use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use trust_runtime::error::RuntimeError;
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{bytecode_bytes_from_source, CompileSession, TestHarness};
use trust_runtime::io::{IoDriver, IoTarget};
use trust_runtime::value::Value;

fn vm_harness(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    let bytes = bytecode_bytes_from_source(source).expect("build bytecode");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply bytecode");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
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
    let mut harness = vm_harness(source_v1);
    let debug = harness.runtime_mut().enable_debug();
    debug.force_global("count", Value::Int(99));
    debug.enqueue_global_write("hold", Value::Int(7));

    let bytes_v2 = bytecode_bytes_from_source(source_v2).expect("build bytecode v2");
    harness
        .runtime_mut()
        .apply_online_change_bytes(&bytes_v2, None)
        .expect("apply online change");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected errors after reload: {:?}",
        cycle.errors
    );

    assert_eq!(
        harness.runtime().storage().get_global("count"),
        Some(&Value::Int(10)),
        "the replacement scan must run the new body from zero, not the cleared force"
    );
    assert_eq!(
        harness.runtime().storage().get_global("hold"),
        Some(&Value::Int(0)),
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
    let session_v1 = CompileSession::from_source(source_v1);
    let session_v2 = CompileSession::from_source(source_v2);
    let mut runtime = session_v1.build_runtime().expect("build runtime");
    let bytes_v1 = session_v1
        .build_bytecode_bytes()
        .expect("build bytecode v1");
    let bytes_v2 = session_v2
        .build_bytecode_bytes()
        .expect("build bytecode v2");
    runtime
        .apply_bytecode_bytes(&bytes_v1, None)
        .expect("apply bytecode v1");
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    runtime
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");

    let reads = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    runtime.add_io_driver(
        "counting",
        Box::new(CountingIoDriver {
            reads: Arc::clone(&reads),
            writes: Arc::clone(&writes),
        }),
    );
    runtime.execute_cycle().expect("cycle before replacement");
    let writes_before = writes.load(Ordering::SeqCst);
    assert!(
        writes_before >= 1,
        "the driver must receive outputs before the replacement"
    );

    runtime
        .apply_online_change_bytes(&bytes_v2, None)
        .expect("apply online change");
    runtime.execute_cycle().expect("cycle after replacement");

    assert!(
        writes.load(Ordering::SeqCst) > writes_before,
        "the surviving driver must receive outputs from the replacement generation"
    );
}

const LEVEL_V1: &str = "VAR_GLOBAL level : DINT := DINT#5; END_VAR \
    PROGRAM Main level := level + DINT#1; END_PROGRAM";
const LEVEL_V2: &str = "VAR_GLOBAL level : DINT := DINT#5; END_VAR \
    PROGRAM Main level := level + DINT#2; END_PROGRAM";

/// Spec 34 §11.3 and UPDATE-04: an online change installs the next generation. A reference taken
/// before it reads nothing and every checked operation refuses it; a reference taken after it
/// works.
#[test]
fn a_reference_taken_before_an_online_change_is_refused_and_a_fresh_one_works() {
    let mut runtime = CompileSession::from_source(LEVEL_V1)
        .build_runtime()
        .expect("build runtime");
    let stale = runtime.storage().ref_for_global("level").expect("level");
    assert_eq!(runtime.read_reference(&stale), Ok(Value::DInt(5)));
    let before = runtime.storage().generation();

    let bytes_v2 = bytecode_bytes_from_source(LEVEL_V2).expect("build bytecode v2");
    runtime
        .apply_online_change_bytes(&bytes_v2, None)
        .expect("apply online change");
    assert_eq!(Some(runtime.storage().generation()), before.next());

    assert_eq!(runtime.storage().read_by_ref_ref(&stale), None);
    assert_eq!(
        runtime.read_reference(&stale),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(
        runtime.write_reference(&stale, Value::DInt(9)),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(runtime.storage().get_global("level"), Some(&Value::DInt(5)));
    assert!(!runtime.faulted());

    let fresh = runtime.storage().ref_for_global("level").expect("level");
    assert_eq!(fresh.generation, runtime.storage().generation());
    runtime
        .write_reference(&fresh, Value::DInt(9))
        .expect("a reference of the installed generation");
    runtime.execute_cycle().expect("cycle after the change");
    assert_eq!(runtime.read_reference(&fresh), Ok(Value::DInt(11)));
}

const ALIASED: &str = "PROGRAM Main VAR member : INT := INT#2; lamp AT %QX0.0 : BOOL; END_VAR \
    lamp := member > INT#5; END_PROGRAM \
    CONFIGURATION C PROGRAM P : Main; \
    VAR_ACCESS Engineering : P.member : INT READ_WRITE; END_VAR \
    END_CONFIGURATION";

/// Every reference the hosted controller holds carries the installed generation.
fn assert_host_references_current(runtime: &trust_runtime::Runtime) {
    let generation = runtime.storage().generation();
    for binding in runtime.access_map().bindings() {
        assert_eq!(binding.reference.generation, generation, "{}", binding.name);
    }
    for binding in runtime.io().bindings() {
        if let IoTarget::Reference(reference) = &binding.target {
            assert_eq!(reference.generation, generation, "{:?}", binding.address);
        }
    }
    for task in runtime.tasks() {
        for reference in &task.fb_instances {
            assert_eq!(reference.generation, generation, "{}", task.name);
        }
    }
}

/// Spec 34 §11.3: a committed restart or online change binds the host's own references, here an
/// engineering alias and an I/O binding record, to the same declarations in the new generation,
/// so they keep working while every reference taken outside goes stale.
#[test]
fn host_references_follow_the_generation_through_restarts_and_online_changes() {
    let mut runtime = CompileSession::from_source(ALIASED)
        .build_runtime()
        .expect("build runtime");
    assert!(
        !runtime.io().bindings().is_empty(),
        "the fixture binds %QX0.0"
    );
    runtime
        .write_access("Engineering", Value::Int(9))
        .expect("alias write");
    assert_eq!(runtime.read_access("Engineering"), Some(Value::Int(9)));

    for mode in [
        trust_runtime::RestartMode::Warm,
        trust_runtime::RestartMode::Cold,
    ] {
        let before = runtime.storage().generation();
        runtime.restart(mode).expect("restart");
        assert_eq!(Some(runtime.storage().generation()), before.next());
        assert_host_references_current(&runtime);
        assert_eq!(runtime.read_access("Engineering"), Some(Value::Int(2)));
        runtime
            .write_access("Engineering", Value::Int(9))
            .expect("alias write after the restart");
        assert_eq!(runtime.read_access("Engineering"), Some(Value::Int(9)));
    }

    let changed = bytecode_bytes_from_source(&ALIASED.replace("INT#5", "INT#6"))
        .expect("build changed bytecode");
    runtime
        .apply_online_change_bytes(&changed, None)
        .expect("apply online change");
    assert_host_references_current(&runtime);
    assert_eq!(runtime.read_access("Engineering"), Some(Value::Int(2)));
    runtime.execute_cycle().expect("cycle after the change");
    assert!(!runtime.faulted());
}
