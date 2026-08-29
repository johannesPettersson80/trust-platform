use indexmap::IndexMap;
use serde_json::json;

use super::*;
use crate::io::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use crate::memory::{FrameId, MemoryLocation, VariableStorage};
use crate::value::{ArrayValue, StructValue, Value, ValueRef};

fn struct_value() -> StructValue {
    StructValue::from_untyped_parts(
        "MotorState".into(),
        IndexMap::from([
            ("enabled".into(), Value::Bool(true)),
            ("speed".into(), Value::DInt(7)),
        ]),
    )
}

fn array_value() -> ArrayValue {
    ArrayValue::from_untyped_parts(
        vec![Value::DInt(10), Value::DInt(20), Value::DInt(30)],
        vec![(1, 3)],
    )
    .expect("array")
}

fn io_entry(name: Option<&str>, address: &str, value: IoSnapshotValue) -> IoSnapshotEntry {
    IoSnapshotEntry {
        name: name.map(Into::into),
        address: IoAddress::parse(address).expect("address"),
        value_type: None,
        value_type_name: None,
        value,
        source: None,
    }
}

#[test]
fn debug_source_uses_camel_case_and_omits_absent_fields() {
    assert_eq!(
        serde_json::to_value(DebugSource {
            name: Some("main.st".into()),
            path: None,
        })
        .expect("serialize source"),
        json!({"name": "main.st"})
    );
}

#[test]
fn debug_scope_uses_camel_case_and_omits_absent_coordinates() {
    assert_eq!(
        serde_json::to_value(DebugScope {
            name: "Globals".into(),
            variables_reference: 7,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
        })
        .expect("serialize scope"),
        json!({
            "name": "Globals",
            "variablesReference": 7,
            "expensive": false
        })
    );
}

#[test]
fn debug_variable_uses_camel_case_and_omits_optional_fields() {
    assert_eq!(
        serde_json::to_value(DebugVariable {
            name: "value".into(),
            value: "7".into(),
            r#type: None,
            variables_reference: 0,
            evaluate_name: None,
        })
        .expect("serialize variable"),
        json!({
            "name": "value",
            "value": "7",
            "variablesReference": 0
        })
    );
}

#[test]
fn variable_handles_start_at_one_and_preserve_kind() {
    let mut handles = DebugVariableHandles::new();
    let locals = handles.alloc(VariableHandle::Locals(FrameId(7)));
    let globals = handles.alloc(VariableHandle::Globals);
    let retain = handles.alloc(VariableHandle::Retain);

    assert_eq!([locals, globals, retain], [1, 2, 3]);
    assert!(matches!(
        handles.get(locals),
        Some(VariableHandle::Locals(FrameId(7)))
    ));
    assert!(matches!(
        handles.get(globals),
        Some(VariableHandle::Globals)
    ));
    assert!(matches!(handles.get(retain), Some(VariableHandle::Retain)));
    assert!(handles.get(0).is_none());
    assert!(handles.get(4).is_none());
}

#[test]
fn variable_handle_clear_invalidates_old_ids_and_restarts_at_one() {
    let mut handles = DebugVariableHandles::new();
    let first = handles.alloc(VariableHandle::Globals);
    let second = handles.alloc(VariableHandle::Retain);
    assert_eq!([first, second], [1, 2]);

    handles.clear();
    assert!(handles.get(first).is_none());
    assert!(handles.get(second).is_none());
    assert_eq!(handles.alloc(VariableHandle::Instances), 1);
}

#[test]
fn variable_handle_allocation_stays_positive_at_counter_boundary() {
    let mut handles = DebugVariableHandles::new();
    handles.next_id = u32::MAX - 1;

    let first = handles.alloc(VariableHandle::Globals);
    let second = handles.alloc(VariableHandle::Retain);

    assert_eq!(first, u32::MAX - 1);
    assert_eq!(second, u32::MAX);
    assert_ne!(first, second);
    assert_ne!(first, 0);
    assert_ne!(second, 0);
}

#[test]
#[should_panic(expected = "debug variable handle space exhausted")]
fn variable_handle_exhaustion_fails_before_aliasing_a_live_handle() {
    let mut handles = DebugVariableHandles::new();
    handles.next_id = u32::MAX;
    let _ = handles.alloc(VariableHandle::Globals);
    let _ = handles.alloc(VariableHandle::Retain);
}

#[test]
fn primitive_value_type_names_use_canonical_iec_spelling() {
    let cases = [
        (Value::Bool(true), "BOOL"),
        (Value::SInt(-1), "SINT"),
        (Value::Int(-2), "INT"),
        (Value::DInt(-3), "DINT"),
        (Value::LInt(-4), "LINT"),
        (Value::USInt(1), "USINT"),
        (Value::UInt(2), "UINT"),
        (Value::UDInt(3), "UDINT"),
        (Value::ULInt(4), "ULINT"),
        (Value::Real(1.5), "REAL"),
        (Value::LReal(2.5), "LREAL"),
        (Value::Byte(1), "BYTE"),
        (Value::Word(2), "WORD"),
        (Value::DWord(3), "DWORD"),
        (Value::LWord(4), "LWORD"),
        (Value::String("text".into()), "STRING"),
        (Value::WString("wide".into()), "WSTRING"),
        (Value::Char(b'A'), "CHAR"),
        (Value::WChar(u16::from(b'B')), "WCHAR"),
        (Value::Instance(InstanceId(7)), "INSTANCE"),
        (Value::Reference(None), "REF"),
        (Value::Null, "NULL"),
    ];

    for (value, expected) in cases {
        assert_eq!(
            value_type_name(&value).as_deref(),
            Some(expected),
            "{value:?}"
        );
    }
}

#[test]
fn compound_type_names_use_public_shape() {
    assert_eq!(
        value_type_name(&Value::Struct(struct_value().into())).as_deref(),
        Some("MotorState")
    );
    assert_eq!(
        value_type_name(&Value::Array(array_value().into())).as_deref(),
        Some("ARRAY")
    );
}

#[test]
fn scalar_variable_has_no_expandable_reference() {
    let mut handles = DebugVariableHandles::new();
    let variable = variable_from_value(
        &mut handles,
        "enabled".into(),
        Value::Bool(true),
        Some("enabled".into()),
    );

    assert_eq!(variable.name, "enabled");
    assert_eq!(variable.value, "TRUE");
    assert_eq!(variable.r#type.as_deref(), Some("BOOL"));
    assert_eq!(variable.variables_reference, 0);
    assert_eq!(variable.evaluate_name.as_deref(), Some("enabled"));
}

#[test]
fn null_and_null_reference_are_not_expandable() {
    let mut handles = DebugVariableHandles::new();
    let null = variable_from_value(&mut handles, "null".into(), Value::Null, None);
    let null_ref = variable_from_value(&mut handles, "ref".into(), Value::Reference(None), None);

    assert_eq!(null.variables_reference, 0);
    assert_eq!(null_ref.variables_reference, 0);
}

#[test]
fn struct_array_and_instance_variables_allocate_typed_handles() {
    let mut handles = DebugVariableHandles::new();
    let structure = variable_from_value(
        &mut handles,
        "motor".into(),
        Value::Struct(struct_value().into()),
        None,
    );
    let array = variable_from_value(
        &mut handles,
        "values".into(),
        Value::Array(array_value().into()),
        None,
    );
    let instance = variable_from_value(
        &mut handles,
        "instance".into(),
        Value::Instance(InstanceId(7)),
        None,
    );

    assert!(matches!(
        handles.get(structure.variables_reference),
        Some(VariableHandle::Struct(value)) if value.type_name() == "MotorState"
    ));
    assert!(matches!(
        handles.get(array.variables_reference),
        Some(VariableHandle::Array(value)) if value.elements().len() == 3
    ));
    assert!(matches!(
        handles.get(instance.variables_reference),
        Some(VariableHandle::Instance(InstanceId(7)))
    ));
}

#[test]
fn nonnull_reference_allocates_reference_handle() {
    let mut handles = DebugVariableHandles::new();
    let value_ref = ValueRef {
        location: MemoryLocation::Global,
        offset: 3,
        path: Vec::new(),
    };
    let variable = variable_from_value(
        &mut handles,
        "reference".into(),
        Value::Reference(Some(value_ref.clone())),
        None,
    );

    assert!(matches!(
        handles.get(variable.variables_reference),
        Some(VariableHandle::Reference(actual)) if actual == &value_ref
    ));
}

#[test]
fn explicit_display_and_type_override_derived_presentation_only() {
    let mut handles = DebugVariableHandles::new();
    let variable = variable_from_value_with_metadata(
        &mut handles,
        "motor".into(),
        Value::Struct(struct_value().into()),
        Some("motor".into()),
        Some("Motor #7".into()),
        Some("PublicMotor".into()),
    );

    assert_eq!(variable.value, "Motor #7");
    assert_eq!(variable.r#type.as_deref(), Some("PublicMotor"));
    assert_ne!(variable.variables_reference, 0);
    assert!(matches!(
        handles.get(variable.variables_reference),
        Some(VariableHandle::Struct(_))
    ));
}

#[test]
fn entry_projection_preserves_input_order_and_evaluate_names() {
    let mut handles = DebugVariableHandles::new();
    let variables = variables_from_entries(
        &mut handles,
        vec![
            ("second".into(), Value::DInt(2)),
            ("first".into(), Value::DInt(1)),
        ],
    );

    assert_eq!(variables.len(), 2);
    assert_eq!(variables[0].name, "second");
    assert_eq!(variables[0].value, "2");
    assert_eq!(variables[0].evaluate_name.as_deref(), Some("second"));
    assert_eq!(variables[1].name, "first");
    assert_eq!(variables[1].evaluate_name.as_deref(), Some("first"));
}

#[test]
fn struct_projection_preserves_declared_field_order() {
    let mut handles = DebugVariableHandles::new();
    let variables = variables_from_struct(&mut handles, struct_value());

    assert_eq!(variables.len(), 2);
    assert_eq!(variables[0].name, "enabled");
    assert_eq!(variables[0].value, "TRUE");
    assert_eq!(variables[1].name, "speed");
    assert_eq!(variables[1].value, "7");
}

#[test]
fn array_projection_uses_zero_based_display_indices_in_element_order() {
    let mut handles = DebugVariableHandles::new();
    let variables = variables_from_array(&mut handles, array_value());

    assert_eq!(variables.len(), 3);
    assert_eq!(variables[0].name, "[0]");
    assert_eq!(variables[0].value, "10");
    assert_eq!(variables[0].evaluate_name.as_deref(), Some("[0]"));
    assert_eq!(variables[2].name, "[2]");
    assert_eq!(variables[2].value, "30");
}

#[test]
fn instance_projection_uses_normalized_id_and_public_type() {
    let mut handles = DebugVariableHandles::new();
    let variables = variables_from_instances(
        &mut handles,
        vec![
            (InstanceId(7), "Motor".into()),
            (InstanceId(u32::MAX), "Valve".into()),
        ],
    );

    assert_eq!(variables[0].name, "Motor#7");
    assert_eq!(variables[0].value, "Motor");
    assert_eq!(variables[0].r#type.as_deref(), Some("Motor"));
    assert_eq!(variables[0].evaluate_name, None);
    assert!(matches!(
        handles.get(variables[0].variables_reference),
        Some(VariableHandle::Instance(InstanceId(7)))
    ));
    assert_eq!(variables[1].name, format!("Valve#{}", u32::MAX));
}

#[test]
fn io_projection_prefers_symbol_name_over_address() {
    let variables = variables_from_io_entries(&[io_entry(
        Some("motor_enable"),
        "%QX0.0",
        IoSnapshotValue::Value(Value::Bool(true)),
    )]);

    assert_eq!(variables.len(), 1);
    assert_eq!(variables[0].name, "motor_enable");
    assert_eq!(variables[0].value, "Bool(true)");
    assert_eq!(variables[0].evaluate_name.as_deref(), Some("motor_enable"));
    assert_eq!(variables[0].variables_reference, 0);
}

#[test]
fn io_projection_formats_each_direct_area_and_size_canonically() {
    let entries = [
        io_entry(None, "%IX1.2", IoSnapshotValue::Unresolved),
        io_entry(None, "%QB3", IoSnapshotValue::Unresolved),
        io_entry(None, "%MW4", IoSnapshotValue::Unresolved),
        io_entry(None, "%ID8", IoSnapshotValue::Unresolved),
        io_entry(None, "%QL16", IoSnapshotValue::Unresolved),
    ];
    let variables = variables_from_io_entries(&entries);

    assert_eq!(
        variables
            .iter()
            .map(|variable| variable.name.as_str())
            .collect::<Vec<_>>(),
        ["%IX1.2", "%QB3", "%MW4", "%ID8", "%QL16"]
    );
}

#[test]
fn io_projection_distinguishes_value_error_and_unresolved() {
    let entries = [
        io_entry(
            Some("value"),
            "%QW0",
            IoSnapshotValue::Value(Value::LInt(7)),
        ),
        io_entry(
            Some("error"),
            "%QW2",
            IoSnapshotValue::Error("offline".into()),
        ),
        io_entry(Some("unresolved"), "%QW4", IoSnapshotValue::Unresolved),
    ];
    let variables = variables_from_io_entries(&entries);

    assert_eq!(variables[0].value, "LInt(7)");
    assert_eq!(variables[1].value, "error: offline");
    assert_eq!(variables[2].value, "unresolved");
}

#[test]
fn io_scope_requires_a_captured_entry_in_any_area() {
    assert!(!io_scope_available(None));
    assert!(!io_scope_available(Some(&IoSnapshot::default())));

    for snapshot in [
        IoSnapshot {
            inputs: vec![io_entry(None, "%IX0.0", IoSnapshotValue::Unresolved)],
            ..IoSnapshot::default()
        },
        IoSnapshot {
            outputs: vec![io_entry(None, "%QX0.0", IoSnapshotValue::Unresolved)],
            ..IoSnapshot::default()
        },
        IoSnapshot {
            memory: vec![io_entry(None, "%MX0.0", IoSnapshotValue::Unresolved)],
            ..IoSnapshot::default()
        },
    ] {
        assert!(io_scope_available(Some(&snapshot)));
    }
}

#[test]
fn instance_display_metadata_uses_live_public_instance_type() {
    let mut storage = VariableStorage::new();
    let id = storage.create_instance("Motor");

    assert_eq!(
        instance_display_metadata(&Value::Instance(id), &storage),
        (Some("Motor".into()), Some("Motor".into()))
    );
}

#[test]
fn instance_display_metadata_is_absent_for_unknown_or_noninstance_value() {
    let storage = VariableStorage::new();
    assert_eq!(
        instance_display_metadata(&Value::Instance(InstanceId(99)), &storage),
        (None, None)
    );
    assert_eq!(
        instance_display_metadata(&Value::DInt(7), &storage),
        (None, None)
    );
}
