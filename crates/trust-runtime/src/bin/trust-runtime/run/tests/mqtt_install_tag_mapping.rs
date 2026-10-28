//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

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
State := Signal#Idle;
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

/// The process-image addresses at which `runtime` binds the variable `name`.
fn bound_addresses(
    runtime: &trust_runtime::CompiledRuntime,
    name: &str,
) -> Vec<trust_runtime::io::IoAddress> {
    runtime.bound_addresses(runtime.find_variable(name).expect("the variable"))
}

/// Spec 32 §5: a reused compatible direct binding keeps its index and its image offset; the mapping
/// adds no binding and does not grow the output image.
#[test]
fn mqtt_install_reused_output_binding_keeps_its_index_and_image_offset() {
    let mut runtime = compiled_program(
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
    );
    let output = trust_runtime::memory::IoArea::Output;
    assert_eq!(runtime.process_image_bytes(output), 4);
    let second = trust_runtime::io::IoAddress::parse("%QW2").unwrap();
    assert_eq!(
        bound_addresses(&runtime, "MainInstance.Second"),
        vec![second.clone()]
    );

    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Second", topic = "line/second", direction = "write" }
]
"#,
    )
    .expect("parse write mapping");
    let (lowered, links) = trust_runtime::io::resolve_compiled_mqtt_tag_mappings(&runtime, &params)
        .expect("reuse the existing %QW binding");

    assert_eq!(
        lowered["output_points"][0]["image_offset"].as_integer(),
        Some(2)
    );
    assert!(links.is_empty(), "a reused binding must not add a binding");
    runtime
        .link_mqtt_tags(&links)
        .expect("install the mapping set");
    let probe = ImageProbe::default();
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime.execute_cycle().expect("drive the outputs");
    assert_eq!(
        probe.outputs().len(),
        4,
        "a reused binding must not grow the output image"
    );
    assert_eq!(
        bound_addresses(&runtime, "MainInstance.Second"),
        vec![second],
        "the binding must still name the same variable at the same address"
    );
}

/// Spec 34 HOST-09 (resource-only binding reconfiguration) and spec 32 §5: installing the mapping set on
/// an existing runtime keeps the PLC storage and does not rerun the initializers.
#[test]
fn mqtt_install_mapping_set_on_an_existing_runtime_keeps_storage_and_skips_initializers() {
    let mut runtime = compiled_program(
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
    );
    runtime.set_current_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("run the program once");
    assert_eq!(
        runtime.read_variable("MainInstance.Green"),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        runtime.read_variable("MainInstance.Red"),
        Ok(Value::Bool(false))
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
    install_tag_mappings(&mut runtime, &params)
        .expect("install the mapping set on the existing runtime");

    assert_eq!(
        runtime.read_variable("MainInstance.Green"),
        Ok(Value::Bool(true)),
        "Green must keep its scanned value"
    );
    assert_eq!(
        runtime.read_variable("MainInstance.Red"),
        Ok(Value::Bool(false)),
        "Red must keep its scanned value rather than its initializer"
    );
    let probe = ImageProbe::default();
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime
        .execute_cycle()
        .expect("copy the kept values into the output image");
    assert_eq!(probe.outputs(), vec![1, 0]);
}

/// Spec 32 §5-6: a reused direct input binding receives the enumeration codec, so an inbound numeric
/// value reconstructs the declared member label.
#[test]
fn mqtt_install_reused_enum_input_binding_reconstructs_member_labels() {
    let mut runtime = compiled_program(ENUM_INPUT_SOURCE);
    let (lowered, links) = trust_runtime::io::resolve_compiled_mqtt_tag_mappings(
        &runtime,
        &install_enum_params("read"),
    )
    .expect("reuse the declared enum input");
    assert!(
        links.is_empty(),
        "the declared input binding must be reused"
    );
    assert_eq!(
        lowered["input_points"][0]["image_offset"].as_integer(),
        Some(0)
    );
    let probe = ImageProbe::driving(&(-1i16).to_le_bytes());
    runtime.add_io_driver("probe", Box::new(probe));
    runtime
        .execute_cycle()
        .expect("decode the reused enum input");
    assert_eq!(
        enum_parts(&runtime.read_variable("MainInstance.State").unwrap()),
        ("Signal".to_string(), "Idle".to_string(), -1),
        "enum input must keep its runtime type"
    );
}

/// Spec 32 §6: an undeclared numeric value on a reused enumeration input is rejected, and the destination
/// keeps its previous value.
#[test]
fn mqtt_install_reused_enum_input_binding_refuses_undeclared_numeric_value() {
    let mut runtime = compiled_program(ENUM_INPUT_SOURCE);
    install_tag_mappings(&mut runtime, &install_enum_params("read"))
        .expect("reuse the declared enum input");
    let probe = ImageProbe::driving(&3i16.to_le_bytes());
    runtime.add_io_driver("probe", Box::new(probe));
    let before = runtime.read_variable("MainInstance.State");

    let error = runtime
        .execute_cycle()
        .expect_err("undeclared enum numeric value must be rejected");

    assert!(error.to_string().contains("enum"), "{error}");
    assert_eq!(runtime.read_variable("MainInstance.State"), before);
}

/// Spec 32 §5-6: a reused direct output binding encodes an enumeration member label to its declared
/// numeric wire value.
#[test]
fn mqtt_install_reused_enum_output_binding_encodes_member_labels() {
    let mut runtime = compiled_program(ENUM_OUTPUT_SOURCE);
    install_tag_mappings(&mut runtime, &install_enum_params("write"))
        .expect("reuse the declared enum output");
    assert_eq!(
        bound_addresses(&runtime, "MainInstance.State"),
        vec![trust_runtime::io::IoAddress::parse("%QW0").unwrap()]
    );
    let probe = ImageProbe::default();
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    // The program assigns the member Idle, whose declared value is -1, in its task's first
    // activation, one interval after its start (specification 10 §4).
    runtime.set_current_time(Duration::from_millis(100));
    runtime
        .execute_cycle()
        .expect("encode the reused enum output");

    assert_eq!(probe.outputs(), (-1i16).to_le_bytes());
}

/// Spec 32 §5: new symbolic points are allocated deterministically after the declared process image.
#[test]
fn mqtt_install_new_points_follow_a_declared_output_word() {
    let mut runtime = compiled_program(
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
    );
    assert_eq!(
        runtime.process_image_bytes(trust_runtime::memory::IoArea::Output),
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

    let lowered = install_tag_mappings(&mut runtime, &params)
        .expect("allocate points after the declared word");

    assert_eq!(
        lowered["output_points"][0]["image_offset"].as_integer(),
        Some(2)
    );
    assert_eq!(
        lowered["output_points"][1]["image_offset"].as_integer(),
        Some(3)
    );
    let probe = ImageProbe::default();
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime.execute_cycle().expect("drive the outputs");
    // The declared word 0, then Green FALSE and Red TRUE, one byte each.
    assert_eq!(probe.outputs(), vec![0, 0, 0, 1]);
}

/// Spec 32 §5 and 34 HOST-09: a refused mapping batch on an installed runtime leaves its bindings, its
/// process image and its PLC storage unchanged.
#[test]
fn mqtt_install_refused_batch_on_an_installed_runtime_leaves_bindings_and_images_unchanged() {
    let mut runtime = compiled_program(
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
    );
    let installed: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" }
]
"#,
    )
    .expect("parse installed mapping");
    install_tag_mappings(&mut runtime, &installed).expect("install the first mapping");
    let probe = ImageProbe::default();
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime.set_current_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("scan the installed mapping");
    assert_eq!(probe.outputs(), vec![1]);

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
    let error = install_tag_mappings(&mut runtime, &refused)
        .expect_err("an unsupported tag must refuse the whole batch");

    assert!(
        matches!(error, trust_runtime::error::RuntimeError::InvalidConfig(_)),
        "{error}"
    );
    assert!(error.to_string().contains("MainInstance.Label"), "{error}");
    runtime
        .execute_cycle()
        .expect("scan after the refused batch");
    assert_eq!(
        probe.outputs(),
        vec![1],
        "the refused batch must not grow or change the output image"
    );
    assert_eq!(
        runtime.read_variable("MainInstance.Green"),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        runtime.read_variable("MainInstance.Red"),
        Ok(Value::Bool(true))
    );
}

/// Spec 32 §5: a tag names one fully qualified program-instance variable, so an unqualified name shared by
/// two instances is a configuration error that installs nothing.
#[test]
fn mqtt_install_unqualified_tag_shared_by_two_instances_is_rejected_atomically() {
    let mut runtime = compiled_program(
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
    );
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "Green", topic = "traffic/green", direction = "write" }
]
"#,
    )
    .expect("parse ambiguous mapping");

    let error = install_tag_mappings(&mut runtime, &params)
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
    assert_installed_nothing(&mut runtime);
}

/// The scan drives an empty output image: no tag was bound.
fn assert_installed_nothing(runtime: &mut trust_runtime::CompiledRuntime) {
    let probe = ImageProbe::default();
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime
        .execute_cycle()
        .expect("scan without the refused mapping");
    assert!(
        probe.outputs().is_empty(),
        "the refused mapping bound an output"
    );
}

/// Spec 32 §5: a non-scalar tag is a configuration error, and no part of the mapping set is installed.
#[test]
fn mqtt_install_non_scalar_tag_is_rejected_atomically() {
    let mut runtime = compiled_program(
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
    );
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Lamps", topic = "line/lamps", direction = "write" }
]
"#,
    )
    .expect("parse non-scalar mapping");

    let error =
        install_tag_mappings(&mut runtime, &params).expect_err("a non-scalar tag must be refused");

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
    assert_installed_nothing(&mut runtime);
}

/// Spec 32 §5-6: a tag outside the MQTT scalar set is a configuration error, and no part of the mapping
/// set is installed.
#[test]
fn mqtt_install_unsupported_tag_type_is_rejected_atomically() {
    let mut runtime = compiled_program(
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
    );
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Speed", topic = "drive/speed", direction = "write" }
]
"#,
    )
    .expect("parse unsupported mapping");

    let error =
        install_tag_mappings(&mut runtime, &params).expect_err("an LREAL tag must be refused");

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
    assert_installed_nothing(&mut runtime);
}
