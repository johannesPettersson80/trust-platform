//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use super::*;

const TRAFFIC_LIGHT: &str = r#"
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
"#;

fn mqtt_driver(params: &str) -> trust_runtime::config::IoDriverConfig {
    trust_runtime::config::IoDriverConfig {
        name: "mqtt".into(),
        enabled: true,
        params: toml::from_str(params).expect("parse MQTT params"),
    }
}

#[test]
fn mqtt_tag_mappings_bind_traffic_light_outputs_before_worker_start() {
    let mut runtime = compiled_program(TRAFFIC_LIGHT);
    let mut bundle = test_bundle();
    bundle.runtime.resource_name = "CommMqttRes".into();
    bundle.io.drivers.push(mqtt_driver(
        r#"
broker = "tcp://127.0.0.1:1883"
# No broker runs in the test: a pending output handoff degrades the driver
# instead of failing the cycle, so the scan still publishes its image.
on_error = "warn"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" },
  { tag = "MainInstance.Yellow", topic = "traffic/north/yellow", direction = "write" },
  { tag = "MainInstance.Red", topic = "traffic/north/red", direction = "write" }
]
"#,
    ));

    super::super::apply_bundle_services(&mut runtime, &bundle)
        .expect("resolve MQTT tag mappings before starting the worker");
    let probe = ImageProbe::default();
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime
        .execute_cycle()
        .expect("copy traffic-light values into the generated output image");

    // One byte per tag, in the mappings' order, after the program's own (empty) output image.
    assert_eq!(
        probe.outputs(),
        vec![0, 0, 1],
        "MQTT tag mappings must bind the outputs in the mappings' order"
    );
}

#[test]
fn mqtt_tag_mapping_lowering_preserves_topics_and_is_atomic_on_unknown_tags() {
    let source = r#"
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
"#;
    let runtime = compiled_program(source);
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
    let (lowered, links) = trust_runtime::io::resolve_compiled_mqtt_tag_mappings(&runtime, &params)
        .expect("lower valid tag mappings");
    let points = lowered["output_points"]
        .as_array()
        .expect("generated output points");
    assert_eq!(points[0]["topic"].as_str(), Some("traffic/north/green"));
    assert_eq!(points[0]["image_offset"].as_integer(), Some(0));
    assert_eq!(points[0]["data_type"].as_str(), Some("bool"));
    assert_eq!(points[1]["topic"].as_str(), Some("traffic/north/red"));
    assert_eq!(points[1]["image_offset"].as_integer(), Some(1));
    assert_eq!(links.len(), 2, "neither variable is bound by the program");

    let mut invalid_runtime = compiled_program(source);
    let invalid = mqtt_driver(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green", direction = "write" },
  { tag = "MainInstance.Missing", topic = "traffic/north/missing", direction = "write" }
]
"#,
    );
    let error = invalid_runtime
        .add_io_config(&trust_runtime::config::IoConfig {
            drivers: vec![invalid],
            safe_state: trust_runtime::io::IoSafeState::default(),
        })
        .expect_err("unknown tag must reject the complete mapping batch");
    assert!(
        error.to_string().contains("MainInstance.Missing"),
        "{error}"
    );
    let probe = ImageProbe::default();
    invalid_runtime.add_io_driver("probe", Box::new(probe.clone()));
    invalid_runtime
        .execute_cycle()
        .expect("scan without the refused batch");
    assert!(
        probe.outputs().is_empty(),
        "the refused batch must bind no output"
    );
}

#[test]
fn mqtt_read_tag_mapping_binds_input_image_to_the_program_variable() {
    let mut runtime = compiled_program(
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
    );
    let params: toml::Value = toml::from_str(
        r#"
broker = "tcp://127.0.0.1:1883"
mappings = [
  { tag = "MainInstance.Green", topic = "traffic/north/green/set", direction = "read" }
]
"#,
    )
    .expect("parse read mapping");

    // This case owns mapping and sampling. Transport freshness is exercised by
    // the controlled-broker I/O tests; no ambient localhost broker is required.
    install_tag_mappings(&mut runtime, &params)
        .expect("resolve read tag mapping without starting a transport worker");
    let probe = ImageProbe::driving(&[1]);
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime
        .execute_cycle()
        .expect("copy generated input into program storage");

    assert_eq!(probe.seen_inputs().len(), 1, "one input byte for the tag");
    assert_eq!(
        runtime.read_variable("MainInstance.Green"),
        Ok(Value::Bool(true))
    );
}

fn enum_mapping_runtime(variable_type: &str) -> trust_runtime::CompiledRuntime {
    compiled_program(&format!(
        r#"
TYPE
    Signal : (Idle := -1, Active := 7);
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
    ))
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
    install_tag_mappings(&mut runtime, &enum_mapping_params("write"))
        .expect("resolve enum write mapping");
    assert_eq!(
        enum_parts(&runtime.read_variable("MainInstance.State").unwrap()),
        ("Signal".to_string(), "Active".to_string(), 7),
        "fixture must exercise a typed enum runtime value"
    );
    let probe = ImageProbe::default();
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime
        .execute_cycle()
        .expect("encode enum output into process image");

    assert_eq!(probe.outputs(), 7i16.to_le_bytes());
}

#[test]
fn mqtt_enum_read_mapping_reconstructs_declared_enum_value() {
    let mut runtime = enum_mapping_runtime("Signal");
    install_tag_mappings(&mut runtime, &enum_mapping_params("read"))
        .expect("resolve enum read mapping");
    let probe = ImageProbe::driving(&(-1i16).to_le_bytes());
    runtime.add_io_driver("probe", Box::new(probe));

    runtime
        .execute_cycle()
        .expect("decode enum input from process image");

    assert_eq!(
        enum_parts(&runtime.read_variable("MainInstance.State").unwrap()),
        ("Signal".to_string(), "Idle".to_string(), -1),
        "enum input must retain its runtime type"
    );
}

#[test]
fn mqtt_enum_read_mapping_rejects_undeclared_numeric_value_atomically() {
    // Specification 32 §6: an undeclared numeric value is rejected before any input binding is
    // committed.
    let mut runtime = enum_mapping_runtime("Signal");
    install_tag_mappings(&mut runtime, &enum_mapping_params("read"))
        .expect("resolve enum read mapping");
    let probe = ImageProbe::driving(&3i16.to_le_bytes());
    runtime.add_io_driver("probe", Box::new(probe));
    let before = runtime.read_variable("MainInstance.State");

    let error = runtime
        .execute_cycle()
        .expect_err("undeclared enum numeric value must be rejected");

    assert!(error.to_string().contains("enum"), "{error}");
    assert_eq!(runtime.read_variable("MainInstance.State"), before);
}

#[test]
fn mqtt_alias_to_enum_mapping_preserves_canonical_enum_identity() {
    // Specification 32 §6: an alias keeps the declared enumeration's identity rather than
    // degrading to the integer base.
    let mut runtime = enum_mapping_runtime("SignalAlias");
    install_tag_mappings(&mut runtime, &enum_mapping_params("read"))
        .expect("resolve alias-to-enum mapping");
    let probe = ImageProbe::driving(&7i16.to_le_bytes());
    runtime.add_io_driver("probe", Box::new(probe));
    runtime.execute_cycle().expect("decode the alias input");

    assert_eq!(
        enum_parts(&runtime.read_variable("MainInstance.State").unwrap()),
        ("Signal".to_string(), "Active".to_string(), 7)
    );
}

#[test]
fn mqtt_integer_named_values_keep_the_full_integer_wire_domain() {
    let source = r#"
TYPE Signal : INT (Idle := -1, Active := 7); SignalAlias : Signal; END_TYPE
PROGRAM TrafficLight
VAR State : SignalAlias := Signal#Active; END_VAR
END_PROGRAM
CONFIGURATION TrafficConfig
RESOURCE CommMqttRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : TrafficLight;
END_RESOURCE
END_CONFIGURATION
"#;
    let mut runtime = compiled_program(source);
    let lowered = install_tag_mappings(&mut runtime, &enum_mapping_params("read")).unwrap();
    assert_eq!(
        lowered["input_points"][0]["data_type"].as_str(),
        Some("i16")
    );
    let probe = ImageProbe::driving(&3i16.to_le_bytes());
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime.execute_cycle().unwrap();
    assert_eq!(probe.seen_inputs().len(), 2, "one INT word for the tag");
    assert_eq!(
        runtime.read_variable("MainInstance.State"),
        Ok(Value::Int(3))
    );
}

#[test]
fn mqtt_enum_mapping_upgrades_an_existing_direct_process_image_binding() {
    let mut runtime = compiled_program(
        r#"
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
"#,
    );
    let (lowered, links) = trust_runtime::io::resolve_compiled_mqtt_tag_mappings(
        &runtime,
        &enum_mapping_params("write"),
    )
    .expect("resolve mapping for direct enum binding");
    assert!(links.is_empty(), "the direct binding is reused");
    assert_eq!(
        lowered["output_points"][0]["image_offset"].as_integer(),
        Some(0)
    );
    let probe = ImageProbe::default();
    runtime.add_io_driver("probe", Box::new(probe.clone()));
    runtime
        .execute_cycle()
        .expect("drive the direct enum output");

    assert_eq!(probe.outputs(), 7i16.to_le_bytes());
}
