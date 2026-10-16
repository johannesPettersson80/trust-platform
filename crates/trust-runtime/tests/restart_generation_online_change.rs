use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use trust_runtime::error::RuntimeError;
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{bytecode_bytes_from_source, CompileSession, TestHarness};
use trust_runtime::io::IoDriver;
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
