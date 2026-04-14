use std::sync::{Arc, Mutex};
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::CompileSession;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

#[test]
fn loads_runtime() {
    let runtime = Runtime::new();
    let _profile = runtime.profile();
}

#[test]
fn runtime_execution_backend_defaults_and_lazy_vm_materialization() {
    let mut runtime = Runtime::new();
    assert_eq!(runtime.execution_backend(), ExecutionBackend::BytecodeVm);
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("vm backend selection should not require preloaded bytecode");

    let source = r#"
        PROGRAM Main
        VAR
            count : DINT := DINT#0;
        END_VAR
        count := count + DINT#1;
        END_PROGRAM
    "#;
    let mut runtime = CompileSession::from_source(source)
        .build_runtime()
        .expect("build runtime");
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    runtime
        .execute_cycle()
        .expect("vm backend should lazy materialize bytecode module");
    let main_id = match runtime.storage().get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main instance global, got {other:?}"),
    };
    assert_eq!(
        runtime.storage().get_instance_var(main_id, "count"),
        Some(&Value::DInt(1))
    );
}

#[test]
fn runtime_metrics_snapshot_tracks_vm_backend_selection() {
    let source = r#"
        PROGRAM Main
        VAR
            count : DINT := DINT#0;
        END_VAR
        count := count + DINT#1;
        END_PROGRAM
    "#;
    let session = CompileSession::from_source(source);
    let mut runtime = session.build_runtime().expect("build runtime");
    let bytes = session.build_bytecode_bytes().expect("build bytecode");
    runtime
        .apply_bytecode_bytes(&bytes, None)
        .expect("apply bytecode");

    let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
    runtime.set_metrics_sink(metrics.clone());
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select vm backend");
    runtime.execute_cycle().expect("execute cycle");

    let snapshot = metrics.lock().expect("metrics lock").snapshot();
    assert_eq!(snapshot.execution_backend, ExecutionBackend::BytecodeVm);
}
