use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::InstanceId;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn artifact(body: &str) -> Vec<u8> {
    let source = format!("{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Tick (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Tick : Main; END_RESOURCE END_CONFIGURATION");
    CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC")
}

fn instance(value: Option<&Value>) -> InstanceId {
    match value {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected instance: {other:?}"),
    }
}

/// Spec 12 §7.8 and spec 10 §2.1.1: a null dereference faults without loading or storing, and the destination of the failed read keeps its value.
#[test]
fn failed_null_dereference_read_leaves_the_destination_unchanged() {
    let bytes = artifact(
        r#"
PROGRAM Main
VAR
    holder : REF_TO INT;
    dest : INT := INT#5;
END_VAR
dest := holder^;
END_PROGRAM
"#,
    );
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();

    assert_eq!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(RuntimeError::NullReference)
    );
    let program = instance(state.storage().get_global("Plant"));
    assert_eq!(
        state.storage().get_instance_var(program, "dest"),
        Some(&Value::Int(5))
    );
}
