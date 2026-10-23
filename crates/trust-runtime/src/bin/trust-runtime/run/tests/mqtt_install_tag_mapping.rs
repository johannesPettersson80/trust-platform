use super::*;

const ENUM_INPUT_SOURCE: &str = r#"
TYPE Signal : (Idle := -1, Active := 7); END_TYPE

PROGRAM EnumProgram
VAR
    State AT %IW0 : Signal := Signal#Active;
END_VAR
END_PROGRAM

CONFIGURATION EnumConfig
RESOURCE EnumResource ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : EnumProgram;
END_RESOURCE
END_CONFIGURATION
"#;

const ENUM_OUTPUT_SOURCE: &str = r#"
TYPE Signal : (Idle := -1, Active := 7); END_TYPE

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
"#;

fn install_enum_params(direction: &str) -> toml::Value {
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

/// Spec 32 §5: a reused compatible direct binding keeps its index and its image offset; the mapping
/// adds no binding and does not grow the output image.
#[test]
fn mqtt_install_reused_output_binding_keeps_its_index_and_image_offset() {
    let session = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM Outputs
VAR
    First AT %QW0 : INT := 0;
    Second AT %QW2 : INT := 0;
END_VAR
END_PROGRAM

CONFIGURATION OutputConfig
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : Outputs;
END_RESOURCE
END_CONFIGURATION
"#,
    )]);
    let mut runtime = session
        .build_runtime()
        .expect("build direct output runtime");
    assert_eq!(runtime.io().bindings().len(), 2);
    assert_eq!(runtime.io().outputs().len(), 4);
    assert_eq!(runtime.io().bindings()[1].address.byte, 2);
    let before = match &runtime.io().bindings()[1].target {
        trust_runtime::io::IoTarget::Reference(reference) => reference.clone(),
        other => panic!("expected reference binding, got {other:?}"),
    };

    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Second", topic = "line/second", direction = "write" }
]
"#,
    )
    .expect("parse write mapping");
    let lowered = trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &params)
        .expect("reuse the existing %QW binding");

    assert_eq!(
        lowered["output_points"][0]["image_offset"].as_integer(),
        Some(2)
    );
    assert_eq!(
        runtime.io().bindings().len(),
        2,
        "a reused binding must not add a binding"
    );
    assert_eq!(
        runtime.io().outputs().len(),
        4,
        "a reused binding must not grow the output image"
    );
    assert_eq!(runtime.io().bindings()[1].address.byte, 2);
    let after = match &runtime.io().bindings()[1].target {
        trust_runtime::io::IoTarget::Reference(reference) => reference.clone(),
        other => panic!("expected reference binding, got {other:?}"),
    };
    assert_eq!(
        after, before,
        "binding index 1 must still name the same variable"
    );
}

/// Spec 34 HOST-09 (resource-only binding reconfiguration) and spec 32 §5: installing the mapping set on
/// an existing runtime keeps the PLC storage and does not rerun the initializers.
#[test]
fn mqtt_install_mapping_set_on_an_existing_runtime_keeps_storage_and_skips_initializers() {
    let session = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM TrafficLight
VAR
    Green : BOOL := FALSE;
    Red : BOOL := TRUE;
END_VAR
Green := TRUE;
Red := FALSE;
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
    runtime.set_current_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("run the program once");
    let instance = match runtime.storage().get_global("MainInstance") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected program instance, got {other:?}"),
    };
    assert_eq!(
        runtime.storage().get_instance_var(instance, "Green"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        runtime.storage().get_instance_var(instance, "Red"),
        Some(&Value::Bool(false))
    );

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
    trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &params)
        .expect("install the mapping set on the existing runtime");

    assert_eq!(
        runtime.storage().get_global("MainInstance"),
        Some(&Value::Instance(instance)),
        "the program instance identity must survive the reconfiguration"
    );
    assert_eq!(
        runtime.storage().get_instance_var(instance, "Green"),
        Some(&Value::Bool(true)),
        "Green must keep its scanned value"
    );
    assert_eq!(
        runtime.storage().get_instance_var(instance, "Red"),
        Some(&Value::Bool(false)),
        "Red must keep its scanned value rather than its initializer"
    );
    assert_eq!(runtime.io().bindings().len(), 2);
    runtime
        .execute_cycle()
        .expect("copy the kept values into the output image");
    assert_eq!(runtime.io().outputs(), &[1, 0]);
}

/// Spec 32 §5-6: a reused direct input binding receives the enumeration codec, so an inbound numeric
/// value reconstructs the declared member label.
#[test]
fn mqtt_install_reused_enum_input_binding_reconstructs_member_labels() {
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        ENUM_INPUT_SOURCE,
    )])
    .build_runtime()
    .expect("build declared enum input runtime");
    assert_eq!(runtime.io().bindings().len(), 1);
    let lowered =
        trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &install_enum_params("read"))
            .expect("reuse the declared enum input");
    assert_eq!(
        runtime.io().bindings().len(),
        1,
        "the declared input binding must be reused"
    );
    assert_eq!(
        lowered["input_points"][0]["image_offset"].as_integer(),
        Some(0)
    );
    runtime
        .write_io(
            &trust_runtime::io::IoAddress::parse("%IW0").unwrap(),
            Value::Word(u16::from_le_bytes((-1i16).to_le_bytes())),
        )
        .unwrap();
    let reference = match &runtime.io().bindings()[0].target {
        trust_runtime::io::IoTarget::Reference(reference) => reference.clone(),
        other => panic!("expected reference binding, got {other:?}"),
    };
    runtime
        .execute_cycle()
        .expect("decode the reused enum input");
    let Some(Value::Enum(value)) = runtime.storage().read_by_ref(reference) else {
        panic!("enum input must keep its runtime type");
    };
    assert_eq!(value.type_name().as_str(), "Signal");
    assert_eq!(value.variant_name().as_str(), "Idle");
    assert_eq!(value.numeric_value(), -1);
}

/// Spec 32 §6: an undeclared numeric value on a reused enumeration input is rejected, and the destination
/// keeps its previous value.
#[test]
fn mqtt_install_reused_enum_input_binding_refuses_undeclared_numeric_value() {
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        ENUM_INPUT_SOURCE,
    )])
    .build_runtime()
    .expect("build declared enum input runtime");
    trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &install_enum_params("read"))
        .expect("reuse the declared enum input");
    runtime
        .write_io(
            &trust_runtime::io::IoAddress::parse("%IW0").unwrap(),
            Value::Word(3),
        )
        .unwrap();
    let reference = match &runtime.io().bindings()[0].target {
        trust_runtime::io::IoTarget::Reference(reference) => reference.clone(),
        other => panic!("expected reference binding, got {other:?}"),
    };
    let before = runtime.storage().read_by_ref(reference.clone()).cloned();

    let error = runtime
        .execute_cycle()
        .expect_err("undeclared enum numeric value must be rejected");

    assert!(error.to_string().contains("enum"), "{error}");
    assert_eq!(runtime.storage().read_by_ref(reference), before.as_ref());
}

/// Spec 32 §5-6: a reused direct output binding encodes an enumeration member label to its declared
/// numeric wire value.
#[test]
fn mqtt_install_reused_enum_output_binding_encodes_member_labels() {
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        ENUM_OUTPUT_SOURCE,
    )])
    .build_runtime()
    .expect("build declared enum output runtime");
    trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &install_enum_params("write"))
        .expect("reuse the declared enum output");
    assert_eq!(runtime.io().bindings().len(), 1);
    let reference = match &runtime.io().bindings()[0].target {
        trust_runtime::io::IoTarget::Reference(reference) => reference.clone(),
        other => panic!("expected reference binding, got {other:?}"),
    };
    let enum_type = runtime.registry().lookup("Signal").expect("Signal type id");
    let idle = trust_runtime::value::EnumValue::new(runtime.registry(), enum_type, "Idle")
        .expect("declared enum label");
    runtime
        .write_reference(&reference, Value::Enum(Box::new(idle)))
        .expect("write declared enum label");
    runtime
        .execute_cycle()
        .expect("encode the reused enum output");

    assert_eq!(runtime.io().outputs(), &(-1i16).to_le_bytes());
}

/// Spec 32 §5: new symbolic points are allocated deterministically after the declared process image.
#[test]
fn mqtt_install_new_points_follow_a_declared_output_word() {
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM Outputs
VAR
    Declared AT %QW0 : INT := 0;
    Green : BOOL := FALSE;
    Red : BOOL := TRUE;
END_VAR
END_PROGRAM

CONFIGURATION OutputConfig
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : Outputs;
END_RESOURCE
END_CONFIGURATION
"#,
    )])
    .build_runtime()
    .expect("build declared-image runtime");
    assert_eq!(
        runtime.io().outputs().len(),
        2,
        "the declared word is the output image before the mapping"
    );
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "line/green", direction = "write" },
  { tag = "MainInstance.Red", topic = "line/red", direction = "write" }
]
"#,
    )
    .expect("parse mappings");

    let lowered = trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &params)
        .expect("allocate points after the declared word");

    assert_eq!(
        lowered["output_points"][0]["image_offset"].as_integer(),
        Some(2)
    );
    assert_eq!(
        lowered["output_points"][1]["image_offset"].as_integer(),
        Some(3)
    );
    assert_eq!(
        runtime.io().bindings().len(),
        3,
        "the declared binding plus one per new point"
    );
    assert_eq!(runtime.io().outputs().len(), 4);
}

/// Spec 32 §5 and 34 HOST-09: a refused mapping batch on an installed runtime leaves its bindings, its
/// process image and its PLC storage unchanged.
#[test]
fn mqtt_install_refused_batch_on_an_installed_runtime_leaves_bindings_and_images_unchanged() {
    let session = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM TrafficLight
VAR
    Green : BOOL := FALSE;
    Red : BOOL := TRUE;
    Label : LREAL := 0.0;
END_VAR
Green := TRUE;
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
    let installed: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" }
]
"#,
    )
    .expect("parse installed mapping");
    trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &installed)
        .expect("install the first mapping");
    runtime.set_current_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("scan the installed mapping");
    assert_eq!(runtime.io().outputs(), &[1]);
    let instance = match runtime.storage().get_global("MainInstance") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected program instance, got {other:?}"),
    };

    let refused: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Red", topic = "traffic/north/red", direction = "write" },
  { tag = "MainInstance.Label", topic = "traffic/north/label", direction = "write" }
]
"#,
    )
    .expect("parse refused batch");
    let error = trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &refused)
        .expect_err("an unsupported tag must refuse the whole batch");

    assert!(
        matches!(error, trust_runtime::error::RuntimeError::InvalidConfig(_)),
        "{error}"
    );
    assert!(error.to_string().contains("MainInstance.Label"), "{error}");
    assert_eq!(
        runtime.io().bindings().len(),
        1,
        "the refused batch must not add a binding"
    );
    assert_eq!(runtime.io().bindings()[0].address.byte, 0);
    assert_eq!(
        runtime.io().outputs(),
        &[1],
        "the refused batch must not grow or change the output image"
    );
    assert_eq!(
        runtime.storage().get_instance_var(instance, "Green"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        runtime.storage().get_instance_var(instance, "Red"),
        Some(&Value::Bool(true))
    );
}

/// Spec 32 §5: a tag names one fully qualified program-instance variable, so an unqualified name shared by
/// two instances is a configuration error that installs nothing.
#[test]
fn mqtt_install_unqualified_tag_shared_by_two_instances_is_rejected_atomically() {
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM NorthProgram
VAR
    Green : BOOL := FALSE;
END_VAR
END_PROGRAM

PROGRAM SouthProgram
VAR
    Green : BOOL := FALSE;
END_VAR
END_PROGRAM

CONFIGURATION TrafficConfig
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM NorthLight WITH MainTask : NorthProgram;
    PROGRAM SouthLight WITH MainTask : SouthProgram;
END_RESOURCE
END_CONFIGURATION
"#,
    )])
    .build_runtime()
    .expect("build two-instance runtime");
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "Green", topic = "traffic/green", direction = "write" }
]
"#,
    )
    .expect("parse ambiguous mapping");

    let error = trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &params)
        .expect_err("an unqualified tag shared by two instances must be refused");

    assert!(
        matches!(error, trust_runtime::error::RuntimeError::InvalidConfig(_)),
        "{error}"
    );
    assert!(
        error.to_string().contains("io.params.mappings[0].tag"),
        "{error}"
    );
    assert!(error.to_string().contains("Green"), "{error}");
    assert!(runtime.io().bindings().is_empty());
    assert!(runtime.io().outputs().is_empty());
}

/// Spec 32 §5: a non-scalar tag is a configuration error, and no part of the mapping set is installed.
#[test]
fn mqtt_install_non_scalar_tag_is_rejected_atomically() {
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM Lights
VAR
    Lamps : ARRAY[0..1] OF BOOL;
END_VAR
END_PROGRAM

CONFIGURATION LightConfig
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : Lights;
END_RESOURCE
END_CONFIGURATION
"#,
    )])
    .build_runtime()
    .expect("build array runtime");
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Lamps", topic = "line/lamps", direction = "write" }
]
"#,
    )
    .expect("parse non-scalar mapping");

    let error = trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &params)
        .expect_err("a non-scalar tag must be refused");

    assert!(
        matches!(error, trust_runtime::error::RuntimeError::InvalidConfig(_)),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .contains("'MainInstance.Lamps' is not a supported MQTT scalar"),
        "{error}"
    );
    assert!(runtime.io().bindings().is_empty());
    assert!(runtime.io().outputs().is_empty());
}

/// Spec 32 §5-6: a tag outside the MQTT scalar set is a configuration error, and no part of the mapping
/// set is installed.
#[test]
fn mqtt_install_unsupported_tag_type_is_rejected_atomically() {
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/main.st",
        r#"
PROGRAM Drive
VAR
    Speed : LREAL := 0.0;
END_VAR
END_PROGRAM

CONFIGURATION DriveConfig
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : Drive;
END_RESOURCE
END_CONFIGURATION
"#,
    )])
    .build_runtime()
    .expect("build LREAL runtime");
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Speed", topic = "drive/speed", direction = "write" }
]
"#,
    )
    .expect("parse unsupported mapping");

    let error = trust_runtime::io::resolve_mqtt_tag_mappings(&mut runtime, &params)
        .expect_err("an LREAL tag must be refused");

    assert!(
        matches!(error, trust_runtime::error::RuntimeError::InvalidConfig(_)),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .contains("'MainInstance.Speed' is not a supported MQTT scalar"),
        "{error}"
    );
    assert!(runtime.io().bindings().is_empty());
    assert!(runtime.io().outputs().is_empty());
}
