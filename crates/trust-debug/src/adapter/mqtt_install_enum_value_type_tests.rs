use super::io::io_state_from_snapshot;
use trust_hir::TypeId;
use trust_runtime::harness::CompileSession;
use trust_runtime::io::{IoAddress, IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use trust_runtime::value::{EnumValue, Value};

const TRAFFIC_STATE_SOURCE: &str = r#"
TYPE TrafficState : (Stopped := -1, Running := 7); END_TYPE

PROGRAM Main
VAR
    state AT %QW0 : TrafficState := TrafficState#Stopped;
END_VAR
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM A WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;

/// Builds a one-entry output snapshot for the declared enumeration `TrafficState`, with the value
/// state chosen by the closure from the runtime registry.
fn traffic_state_snapshot(
    make_value: impl FnOnce(&trust_runtime::Runtime, TypeId) -> IoSnapshotValue,
) -> IoSnapshot {
    let runtime = CompileSession::from_source(TRAFFIC_STATE_SOURCE)
        .build_runtime()
        .expect("build traffic state runtime");
    let enum_type = runtime
        .registry()
        .lookup("TrafficState")
        .expect("TrafficState type id");
    let value = make_value(&runtime, enum_type);
    IoSnapshot {
        outputs: vec![IoSnapshotEntry {
            name: Some("A.state".into()),
            address: IoAddress::parse("%QW0").expect("enum output address"),
            value_type: Some(enum_type),
            value_type_name: Some("TrafficState".into()),
            value,
            source: None,
        }],
        ..IoSnapshot::default()
    }
}

/// Spec 32 §6: a resolved enumeration value keeps its declared type name in the DAP I/O state.
#[test]
fn mqtt_install_dap_io_state_keeps_declared_enum_value_type_when_resolved() {
    let snapshot = traffic_state_snapshot(|runtime, enum_type| {
        IoSnapshotValue::Value(Value::Enum(Box::new(
            EnumValue::new(runtime.registry(), enum_type, "Running").expect("declared enum value"),
        )))
    });

    let body = io_state_from_snapshot(snapshot);

    assert_eq!(body.outputs.len(), 1);
    assert_eq!(body.outputs[0].value_type.as_deref(), Some("TrafficState"));
}

/// Spec 32 §6: an unresolved enumeration value keeps its declared type name in the DAP I/O state.
#[test]
fn mqtt_install_dap_io_state_keeps_declared_enum_value_type_when_unresolved() {
    let snapshot = traffic_state_snapshot(|_, _| IoSnapshotValue::Unresolved);

    let body = io_state_from_snapshot(snapshot);

    assert_eq!(body.outputs.len(), 1);
    assert_eq!(body.outputs[0].value_type.as_deref(), Some("TrafficState"));
}

/// Spec 32 §6: an I/O conversion error keeps the declared enumeration type name in the DAP I/O state.
#[test]
fn mqtt_install_dap_io_state_keeps_declared_enum_value_type_on_conversion_error() {
    let snapshot = traffic_state_snapshot(|_, _| {
        IoSnapshotValue::Error("undeclared enum wire value 9".to_string())
    });

    let body = io_state_from_snapshot(snapshot);

    assert_eq!(body.outputs.len(), 1);
    assert_eq!(body.outputs[0].value_type.as_deref(), Some("TrafficState"));
}
