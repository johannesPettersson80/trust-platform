//! Completed function payloads retain copyback and recycling for legal ST connections.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

#[test]
fn aggregate_outputs_keep_values_across_recycled_calls_and_restart() {
    let source = r#"
TYPE Payload : ARRAY[0..255] OF DINT; END_TYPE
VAR_GLOBAL target : Payload; published : Payload; total : DINT; END_VAR
FUNCTION Update : DINT
VAR_IN_OUT first : Payload; END_VAR
VAR_OUTPUT second : Payload; END_VAR
second := first;
first[0] := first[0] + 1;
second[0] := second[0] + 2;
Update := first[0];
END_FUNCTION
PROGRAM Main
VAR_EXTERNAL target : Payload; published : Payload; total : DINT; END_VAR
total := Update(first := target, second => published);
END_PROGRAM
"#;
    let artifact = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared =
        PreparedModule::from_bytes(&artifact.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    for sample in 1..=3 {
        state
            .execute_cycle(Duration::from_millis(sample * 10))
            .unwrap();
        let Some(Value::Array(target)) = state.storage().get_global("target") else {
            panic!("array destination");
        };
        assert_eq!(target.elements()[0], Value::DInt(sample as i32));
        assert!(target.elements()[1..]
            .iter()
            .all(|value| *value == Value::DInt(0)));
        let Some(Value::Array(published)) = state.storage().get_global("published") else {
            panic!("independent output destination");
        };
        assert_eq!(published.elements()[0], Value::DInt(sample as i32 + 1));
        assert!(published.elements()[1..]
            .iter()
            .all(|value| *value == Value::DInt(0)));
        assert_eq!(
            state.storage().get_global("total"),
            Some(&Value::DInt(sample as i32))
        );
    }
    state.restart(RestartMode::Cold).unwrap();
    state.execute_cycle(Duration::from_millis(40)).unwrap();
    let Some(Value::Array(target)) = state.storage().get_global("target") else {
        panic!("array destination after restart");
    };
    assert_eq!(target.elements()[0], Value::DInt(1));
    let Some(Value::Array(published)) = state.storage().get_global("published") else {
        panic!("independent output destination after restart");
    };
    assert_eq!(published.elements()[0], Value::DInt(2));
    assert_eq!(state.storage().get_global("total"), Some(&Value::DInt(1)));
}
