//! Saved current-format execution complements the explicit legacy wire oracles.

use super::*;

#[test]
fn saved_source_free_parameter_defaults_and_retained_initializers_execute() {
    use trust_runtime::value::Value;
    let source = r#"
VAR_GLOBAL RETAIN saved : INT := INT#7; END_VAR
FUNCTION AddDefault : INT
VAR_INPUT amount : INT := INT#5; END_VAR
AddDefault := amount + INT#1;
END_FUNCTION
PROGRAM Main
VAR omitted : INT; supplied : INT; END_VAR
omitted := AddDefault();
supplied := AddDefault(amount := INT#10);
END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module()
        .unwrap();
    assert_eq!(module.version, BytecodeVersion::SOURCE_FREE);
    assert!(module.section(SectionId::RetainInit).is_none());
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("missing source-free storage declarations");
    };
    assert!(layout.entries.iter().any(|entry| entry.is_retained()));
    let bytes = module.encode().unwrap();
    let mut runtime = trust_runtime::Runtime::from_bytes(&bytes, Default::default())
        .expect("saved artifact constructs state without source context");
    assert_eq!(runtime.storage().get_global("saved"), Some(&Value::Int(7)));
    runtime.execute_cycle().unwrap();
    let Some(Value::Instance(main)) = runtime.storage().get_global("Main") else {
        panic!("missing program root");
    };
    assert_eq!(
        runtime.storage().get_instance_var(*main, "omitted"),
        Some(&Value::Int(6))
    );
    assert_eq!(
        runtime.storage().get_instance_var(*main, "supplied"),
        Some(&Value::Int(11))
    );
    runtime.write_global("saved", Value::Int(9)).unwrap();
    runtime.restart(trust_runtime::RestartMode::Warm).unwrap();
    assert_eq!(runtime.storage().get_global("saved"), Some(&Value::Int(9)));
    runtime.restart(trust_runtime::RestartMode::Cold).unwrap();
    assert_eq!(runtime.storage().get_global("saved"), Some(&Value::Int(7)));
}
