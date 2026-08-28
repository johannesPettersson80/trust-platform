use super::*;

#[test]
fn mqtt_tag_mappings_bind_traffic_light_outputs_before_worker_start() {
    let session = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM TrafficLight
VAR
    Green : BOOL := FALSE;
    Yellow : BOOL := FALSE;
    Red : BOOL := TRUE;
END_VAR
END_PROGRAM

CONFIGURATION TrafficConfig
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : TrafficLight;
END_RESOURCE
END_CONFIGURATION
"#,
    )]);
    let mut runtime = session
        .build_runtime()
        .expect("build traffic-light runtime");
    let mut bundle =
        bundle_with_backend(trust_runtime::execution_backend::ExecutionBackend::BytecodeVm);
    bundle.runtime.resource_name = "CommMqttRes".into();
    bundle.bytecode = session
        .build_bytecode_bytes()
        .expect("build traffic-light bytecode");
    bundle
        .io
        .drivers
        .push(trust_runtime::config::IoDriverConfig {
            name: "mqtt".into(),
            enabled: true,
            params: toml::from_str(
                r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" },
  { tag = "MainInstance.Yellow", topic = "traffic/north/yellow", direction = "write" },
  { tag = "MainInstance.Red", topic = "traffic/north/red", direction = "write" }
]
"#,
            )
            .expect("parse MQTT params"),
        });

    super::super::apply_bundle_runtime_overrides(&mut runtime, &bundle)
        .expect("resolve MQTT tag mappings before starting the worker");

    let names = runtime
        .io()
        .bindings()
        .iter()
        .filter_map(|binding| binding.display_name.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "MainInstance.Green",
            "MainInstance.Yellow",
            "MainInstance.Red"
        ],
        "MQTT tag mappings must create process-image bindings"
    );
    assert_eq!(runtime.io().outputs().len(), 3);
    let storage = runtime.storage().clone();
    runtime
        .io_mut()
        .write_outputs(&storage)
        .expect("copy traffic-light values into the generated output image");
    assert_eq!(runtime.io().outputs(), &[0, 0, 1]);
}

#[test]
fn mqtt_tag_mapping_lowering_preserves_topics_and_is_atomic_on_unknown_tags() {
    let session = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM TrafficLight
VAR
    Green : BOOL;
    Red : BOOL := TRUE;
END_VAR
END_PROGRAM

CONFIGURATION TrafficConfig
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : TrafficLight;
END_RESOURCE
END_CONFIGURATION
"#,
    )]);
    let mut runtime = session
        .build_runtime()
        .expect("build traffic-light runtime");
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" },
  { tag = "MainInstance.Red", topic = "traffic/north/red", direction = "write" }
]
"#,
    )
    .expect("parse mappings");
    let lowered = trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &params)
        .expect("lower valid tag mappings");
    let points = lowered["output_points"]
        .as_array()
        .expect("generated output points");
    assert_eq!(points[0]["topic"].as_str(), Some("traffic/north/green"));
    assert_eq!(points[0]["image_offset"].as_integer(), Some(0));
    assert_eq!(points[0]["data_type"].as_str(), Some("bool"));
    assert_eq!(points[1]["topic"].as_str(), Some("traffic/north/red"));
    assert_eq!(points[1]["image_offset"].as_integer(), Some(1));

    let mut invalid_runtime = session.build_runtime().expect("build atomicity runtime");
    let invalid: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" },
  { tag = "MainInstance.Missing", topic = "traffic/north/missing", direction = "write" }
]
"#,
    )
    .expect("parse invalid mappings");
    let error = trust_runtime::io::resolve_mqtt_tag_mappings(&mut invalid_runtime, &invalid)
        .expect_err("unknown tag must reject the complete mapping batch");
    assert!(
        error.to_string().contains("MainInstance.Missing"),
        "{error}"
    );
    assert!(invalid_runtime.io().bindings().is_empty());
    assert!(invalid_runtime.io().outputs().is_empty());
}

#[test]
fn mqtt_read_tag_mapping_binds_broker_input_to_the_program_variable() {
    let session = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM TrafficLight
VAR
    Green : BOOL := FALSE;
END_VAR
END_PROGRAM

CONFIGURATION TrafficConfig
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : TrafficLight;
END_RESOURCE
END_CONFIGURATION
"#,
    )]);
    let mut runtime = session
        .build_runtime()
        .expect("build traffic-light runtime");
    let mut bundle =
        bundle_with_backend(trust_runtime::execution_backend::ExecutionBackend::BytecodeVm);
    bundle.runtime.resource_name = "CommMqttRes".into();
    bundle.bytecode = session
        .build_bytecode_bytes()
        .expect("build traffic-light bytecode");
    bundle
        .io
        .drivers
        .push(trust_runtime::config::IoDriverConfig {
            name: "mqtt".into(),
            enabled: true,
            params: toml::from_str(
                r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green/set", direction = "read" }
]
"#,
            )
            .expect("parse read mapping"),
        });

    super::super::apply_bundle_runtime_overrides(&mut runtime, &bundle)
        .expect("resolve read tag mapping before starting the worker");

    assert_eq!(runtime.io().inputs().len(), 1);
    runtime.io_mut().inputs_mut()[0] = 1;
    let reference = match &runtime.io().bindings()[0].target {
        trust_runtime::io::IoTarget::Reference(reference) => reference.clone(),
        other => panic!("expected reference binding, got {other:?}"),
    };
    let mut storage = runtime.storage().clone();
    runtime
        .io()
        .read_inputs(&mut storage)
        .expect("copy generated input into program storage");
    assert_eq!(
        storage.read_by_ref(reference),
        Some(&trust_runtime::value::Value::Bool(true))
    );
}

fn enum_mapping_runtime(variable_type: &str) -> trust_runtime::Runtime {
    let source = format!(
        r#"
TYPE
    Signal : INT (Idle := -1, Active := 7);
    SignalAlias : Signal;
END_TYPE

PROGRAM EnumProgram
VAR
    State : {variable_type} := Signal#Active;
END_VAR
END_PROGRAM

CONFIGURATION EnumConfig
RESOURCE EnumResource ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : EnumProgram;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    CompileSession::from_sources(vec![SourceFile::with_path("src/main.st", source)])
        .build_runtime()
        .expect("build enum mapping runtime")
}

fn enum_mapping_params(direction: &str) -> toml::Value {
    toml::from_str(&format!(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  {{ tag = "MainInstance.State", topic = "line/state", direction = "{direction}" }}
]
"#,
    ))
    .expect("parse enum mapping")
}

#[test]
fn mqtt_enum_write_mapping_uses_numeric_iec_wire_value() {
    let mut runtime = enum_mapping_runtime("Signal");
    trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &enum_mapping_params("write"))
        .expect("resolve enum write mapping");

    let mut storage = runtime.storage().clone();
    let reference = match &runtime.io().bindings()[0].target {
        trust_runtime::io::IoTarget::Reference(reference) => reference.clone(),
        other => panic!("expected reference binding, got {other:?}"),
    };
    let enum_type = runtime.registry().lookup("Signal").expect("Signal type id");
    let enum_value = trust_runtime::value::EnumValue::new(runtime.registry(), enum_type, "Active")
        .expect("declared enum value");
    assert!(storage.write_by_ref(
        reference.clone(),
        trust_runtime::value::Value::Enum(Box::new(enum_value))
    ));
    assert!(
        matches!(
            storage.read_by_ref(reference),
            Some(trust_runtime::value::Value::Enum(_))
        ),
        "fixture must exercise a typed enum runtime value"
    );
    runtime
        .io_mut()
        .write_outputs(&storage)
        .expect("encode enum output into process image");

    assert_eq!(runtime.io().outputs(), &7i16.to_le_bytes());
}

#[test]
fn mqtt_enum_read_mapping_reconstructs_declared_enum_value() {
    let mut runtime = enum_mapping_runtime("Signal");
    trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &enum_mapping_params("read"))
        .expect("resolve enum read mapping");
    runtime
        .io_mut()
        .inputs_mut()
        .copy_from_slice(&(-1i16).to_le_bytes());
    let reference = match &runtime.io().bindings()[0].target {
        trust_runtime::io::IoTarget::Reference(reference) => reference.clone(),
        other => panic!("expected reference binding, got {other:?}"),
    };
    let mut storage = runtime.storage().clone();

    runtime
        .io()
        .read_inputs(&mut storage)
        .expect("decode enum input from process image");

    let Some(trust_runtime::value::Value::Enum(value)) = storage.read_by_ref(reference) else {
        panic!("enum input must retain its runtime type");
    };
    assert_eq!(value.type_name().as_str(), "Signal");
    assert_eq!(value.variant_name().as_str(), "Idle");
    assert_eq!(value.numeric_value(), -1);
}

#[test]
fn mqtt_enum_read_mapping_rejects_undeclared_numeric_value_atomically() {
    let mut runtime = enum_mapping_runtime("Signal");
    trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &enum_mapping_params("read"))
        .expect("resolve enum read mapping");
    runtime
        .io_mut()
        .inputs_mut()
        .copy_from_slice(&3i16.to_le_bytes());
    let reference = match &runtime.io().bindings()[0].target {
        trust_runtime::io::IoTarget::Reference(reference) => reference.clone(),
        other => panic!("expected reference binding, got {other:?}"),
    };
    let mut storage = runtime.storage().clone();
    let before = storage.read_by_ref(reference.clone()).cloned();

    let error = runtime
        .io()
        .read_inputs(&mut storage)
        .expect_err("undeclared enum numeric value must be rejected");

    assert!(error.to_string().contains("enum"), "{error}");
    assert_eq!(storage.read_by_ref(reference), before.as_ref());
}

#[test]
fn mqtt_alias_to_enum_mapping_preserves_canonical_enum_identity() {
    let mut runtime = enum_mapping_runtime("SignalAlias");
    trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &enum_mapping_params("write"))
        .expect("resolve alias-to-enum mapping");
    let enum_type = runtime.registry().lookup("Signal").expect("Signal type id");

    assert_eq!(runtime.io().bindings()[0].value_type, Some(enum_type));
}

#[test]
fn mqtt_enum_mapping_upgrades_an_existing_direct_process_image_binding() {
    let session = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
TYPE Signal : INT (Idle := -1, Active := 7); END_TYPE

PROGRAM EnumProgram
VAR
    State AT %QW0 : Signal := Signal#Active;
END_VAR
END_PROGRAM

CONFIGURATION EnumConfig
RESOURCE EnumResource ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : EnumProgram;
END_RESOURCE
END_CONFIGURATION
"#,
    )]);
    let mut runtime = session.build_runtime().expect("build direct enum runtime");
    trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &enum_mapping_params("write"))
        .expect("resolve mapping for direct enum binding");
    let enum_type = runtime.registry().lookup("Signal").expect("Signal type id");

    assert_eq!(runtime.io().bindings().len(), 1);
    assert_eq!(runtime.io().bindings()[0].value_type, Some(enum_type));
}
