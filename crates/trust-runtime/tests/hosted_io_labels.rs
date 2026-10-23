//! Hosted presentation labels survive declared construction and saved-artifact loading.
use trust_runtime::{
    harness::CompileSession,
    io::{IoAddress, IoSnapshotValue},
    value::Value,
    Runtime, RuntimeBuilder,
};

fn assert_input(runtime: &Runtime, label: &str) {
    let snapshot = runtime.io().snapshot();
    assert_eq!(snapshot.inputs.len(), 1);
    assert_eq!(snapshot.inputs[0].name.as_deref(), Some(label));
    assert!(matches!(
        snapshot.inputs[0].value,
        IoSnapshotValue::Value(Value::Bool(true))
    ));
}

#[test]
fn manual_named_binding_keeps_its_name_and_value_through_install_and_saved_load() {
    let mut builder = RuntimeBuilder::new();
    builder
        .register_global(trust_runtime::program_model::VarDef {
            name: "IN0".into(),
            type_id: trust_hir::TypeId::BOOL,
            initializer: None,
            retain: trust_runtime::RetainPolicy::Unspecified,
            static_storage: false,
            external: false,
            in_out: false,
            constant: false,
            address: None,
        })
        .unwrap();
    let address = IoAddress::parse("%IX0.0").unwrap();
    builder.io_mut().bind("IN0", address.clone());
    builder.io_mut().write(&address, Value::Bool(true)).unwrap();
    let runtime = builder.build().unwrap();
    assert_input(&runtime, "IN0");
    let module = trust_runtime::bytecode::build_module_from_runtime(&runtime).unwrap();
    let restored = Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap();
    assert_input(&restored, "IN0");
}

#[test]
fn configured_symbolic_binding_retains_qualified_path_for_host_and_saved_load() {
    let source = r#"
PROGRAM Main
VAR enabled : BOOL; END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG P.enabled AT %IX0.0 : BOOL; END_VAR
END_CONFIGURATION
"#;
    let mut runtime = CompileSession::from_source(source).build_runtime().unwrap();
    let address = IoAddress::parse("%IX0.0").unwrap();
    runtime.write_io(&address, Value::Bool(true)).unwrap();
    assert_input(&runtime, "P.enabled");
    let module = trust_runtime::bytecode::build_module_from_runtime(&runtime).unwrap();
    let mut restored = Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap();
    restored.write_io(&address, Value::Bool(true)).unwrap();
    assert_input(&restored, "P.enabled");
}
