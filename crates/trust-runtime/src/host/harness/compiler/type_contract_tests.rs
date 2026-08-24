use crate::harness::CompileSession;
use crate::program_model::Expr;
use crate::value::Value;
use crate::Runtime;
use trust_hir::{Type, TypeId};

fn runtime(source: &str) -> Runtime {
    CompileSession::from_source(source)
        .build_runtime()
        .unwrap_or_else(|error| panic!("type fixture must compile: {error}"))
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("type fixture must fail"),
        Err(error) => error.to_string(),
    }
}

fn type_id(runtime: &Runtime, name: &str) -> TypeId {
    runtime
        .registry()
        .lookup(name)
        .unwrap_or_else(|| panic!("missing type {name}"))
}

fn type_def<'a>(runtime: &'a Runtime, name: &str) -> &'a Type {
    let id = type_id(runtime, name);
    runtime
        .registry()
        .get(id)
        .unwrap_or_else(|| panic!("missing definition for {name}"))
}

#[test]
fn type_contract_direct_alias_retains_name_target_and_case_insensitive_lookup() {
    let runtime = runtime(
        r#"
TYPE BatchCount : DINT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let id = type_id(&runtime, "batchcount");
    assert_eq!(runtime.registry().lookup("BATCHCOUNT"), Some(id));
    assert!(matches!(
        runtime.registry().get(id),
        Some(Type::Alias { name, target })
            if name == "BatchCount" && *target == TypeId::DINT
    ));
}

#[test]
fn type_contract_namespace_qualifies_canonical_type_name() {
    let runtime = runtime(
        r#"
NAMESPACE Cell
TYPE StateCode : INT;
END_TYPE
END_NAMESPACE
PROGRAM Main
END_PROGRAM
"#,
    );
    let id = type_id(&runtime, "cell.statecode");
    assert_eq!(
        runtime.registry().type_name(id).as_deref(),
        Some("Cell.StateCode")
    );
    assert!(runtime.registry().lookup("StateCode").is_none());
}

#[test]
fn type_contract_using_resolves_unqualified_alias_target() {
    let runtime = runtime(
        r#"
NAMESPACE Shared
TYPE CounterValue : DINT;
END_TYPE
END_NAMESPACE
USING Shared;
TYPE LocalCounter : CounterValue;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let shared = type_id(&runtime, "Shared.CounterValue");
    assert!(matches!(
        type_def(&runtime, "LocalCounter"),
        Type::Alias { target, .. } if *target == shared
    ));
}

#[test]
fn type_contract_subrange_retains_base_and_inclusive_bounds() {
    let runtime = runtime(
        r#"
TYPE SignedWindow : INT(-5..12);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let alias_target = match type_def(&runtime, "SignedWindow") {
        Type::Alias { target, .. } => *target,
        other => panic!("expected subrange alias, got {other:?}"),
    };
    assert!(matches!(
        runtime.registry().get(alias_target),
        Some(Type::Subrange {
            base: TypeId::INT,
            lower: -5,
            upper: 12
        })
    ));
}

#[test]
fn type_contract_array_retains_element_and_declared_bounds() {
    let runtime = runtime(
        r#"
TYPE Samples : ARRAY[-2..2] OF DINT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let target = match type_def(&runtime, "Samples") {
        Type::Alias { target, .. } => *target,
        other => panic!("expected array alias, got {other:?}"),
    };
    assert!(matches!(
        runtime.registry().get(target),
        Some(Type::Array {
            element: TypeId::DINT,
            dimensions
        }) if dimensions.as_slice() == [(-2, 2)]
    ));
}

#[test]
fn type_contract_multidimensional_array_retains_source_dimension_order() {
    let runtime = runtime(
        r#"
TYPE Grid : ARRAY[1..2, -1..1, 4..6] OF BOOL;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let target = match type_def(&runtime, "Grid") {
        Type::Alias { target, .. } => *target,
        other => panic!("expected array alias, got {other:?}"),
    };
    assert!(matches!(
        runtime.registry().get(target),
        Some(Type::Array {
            element: TypeId::BOOL,
            dimensions
        }) if dimensions.as_slice() == [(1, 2), (-1, 1), (4, 6)]
    ));
}

#[test]
fn type_contract_array_resolves_a_preceding_user_element_type() {
    let runtime = runtime(
        r#"
TYPE
ItemCode : UINT;
ItemBuffer : ARRAY[0..3] OF ItemCode;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let item = type_id(&runtime, "ItemCode");
    let target = match type_def(&runtime, "ItemBuffer") {
        Type::Alias { target, .. } => *target,
        other => panic!("expected array alias, got {other:?}"),
    };
    assert!(matches!(
        runtime.registry().get(target),
        Some(Type::Array {
            element,
            dimensions
        }) if *element == item && dimensions.as_slice() == [(0, 3)]
    ));
}

#[test]
fn type_contract_struct_retains_field_and_multi_name_source_order() {
    let runtime = runtime(
        r#"
TYPE Packet : STRUCT
    Enabled : BOOL;
    First, Second : INT;
    Sequence : UDINT;
END_STRUCT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let fields = match type_def(&runtime, "Packet") {
        Type::Struct { fields, .. } => fields,
        other => panic!("expected struct, got {other:?}"),
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| (field.name.as_str(), field.type_id))
            .collect::<Vec<_>>(),
        [
            ("Enabled", TypeId::BOOL),
            ("First", TypeId::INT),
            ("Second", TypeId::INT),
            ("Sequence", TypeId::UDINT)
        ]
    );
}

#[test]
fn type_contract_struct_retains_relative_member_addresses() {
    let runtime = runtime(
        r#"
TYPE WireImage : STRUCT
    Header AT %B0 : INT;
    Ready AT %X2.0 : BOOL;
END_STRUCT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let fields = match type_def(&runtime, "WireImage") {
        Type::Struct { fields, .. } => fields,
        other => panic!("expected struct, got {other:?}"),
    };
    assert_eq!(fields[0].address.as_deref(), Some("%B0"));
    assert_eq!(fields[1].address.as_deref(), Some("%X2.0"));
}

#[test]
fn type_contract_struct_member_default_is_bound_to_exact_field() {
    let runtime = runtime(
        r#"
TYPE Settings : STRUCT
    Limit : INT := INT#7;
    Enabled : BOOL;
END_STRUCT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let fields = match type_def(&runtime, "Settings") {
        Type::Struct { fields, .. } => fields,
        other => panic!("expected struct, got {other:?}"),
    };
    let initializer = fields[0]
        .default_initializer
        .expect("Limit default initializer");
    assert!(fields[1].default_initializer.is_none());
    assert!(matches!(
        runtime.initializer_catalog().initializer(initializer),
        Some(Expr::Literal(Value::Int(7)))
    ));
}

#[test]
fn type_contract_multi_name_members_share_declared_default_expression() {
    let runtime = runtime(
        r#"
TYPE Pair : STRUCT
    Left, Right : INT := INT#3;
END_STRUCT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let fields = match type_def(&runtime, "Pair") {
        Type::Struct { fields, .. } => fields,
        other => panic!("expected struct, got {other:?}"),
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].default_initializer, fields[1].default_initializer);
    let initializer = fields[0]
        .default_initializer
        .expect("shared member initializer");
    assert!(matches!(
        runtime.initializer_catalog().initializer(initializer),
        Some(Expr::Literal(Value::Int(3)))
    ));
}

#[test]
fn type_contract_self_reference_targets_reserved_struct_identity() {
    let runtime = runtime(
        r#"
TYPE Node : STRUCT
    Value : INT;
    Next : REF_TO Node;
END_STRUCT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let node = type_id(&runtime, "Node");
    let fields = match runtime.registry().get(node) {
        Some(Type::Struct { fields, .. }) => fields,
        other => panic!("expected Node struct, got {other:?}"),
    };
    let reference = runtime
        .registry()
        .get(fields[1].type_id)
        .expect("Next reference type");
    assert!(matches!(reference, Type::Reference { target } if *target == node));
}

#[test]
fn type_contract_union_retains_variant_order_types_addresses_and_defaults() {
    let runtime = runtime(
        r#"
TYPE Payload : UNION
    WordValue AT %B0 : WORD := WORD#16#1234;
    Flag AT %X0.0 : BOOL;
END_UNION;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let variants = match type_def(&runtime, "Payload") {
        Type::Union { variants, .. } => variants,
        other => panic!("expected union, got {other:?}"),
    };
    assert_eq!(
        variants
            .iter()
            .map(|variant| (variant.name.as_str(), variant.type_id))
            .collect::<Vec<_>>(),
        [("WordValue", TypeId::WORD), ("Flag", TypeId::BOOL)]
    );
    assert_eq!(variants[0].address.as_deref(), Some("%B0"));
    assert_eq!(variants[1].address.as_deref(), Some("%X0.0"));
    let initializer = variants[0]
        .default_initializer
        .expect("union member initializer");
    assert!(matches!(
        runtime.initializer_catalog().initializer(initializer),
        Some(Expr::Literal(Value::Word(0x1234)))
    ));
}

#[test]
fn type_contract_enum_uses_int_base_and_zero_based_implicit_values() {
    let runtime = runtime(
        r#"
TYPE Phase : (Idle, Running, Complete);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(matches!(
        type_def(&runtime, "Phase"),
        Type::Enum { base, values, .. }
            if *base == TypeId::INT
                && values.as_slice()
                    == [
                        ("Idle".into(), 0),
                        ("Running".into(), 1),
                        ("Complete".into(), 2)
                    ]
    ));
}

#[test]
fn type_contract_enum_continues_after_explicit_values() {
    let runtime = runtime(
        r#"
TYPE Phase : (Idle := 4, Running, Complete := 9, Failed);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let values = match type_def(&runtime, "Phase") {
        Type::Enum { values, .. } => values,
        other => panic!("expected enum, got {other:?}"),
    };
    assert_eq!(
        values.as_slice(),
        [
            ("Idle".into(), 4),
            ("Running".into(), 5),
            ("Complete".into(), 9),
            ("Failed".into(), 10)
        ]
    );
}

#[test]
fn type_contract_enum_retains_explicit_integer_base() {
    let runtime = runtime(
        r#"
TYPE ResultCode : DINT (Ready := 10, Busy := 20);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(matches!(
        type_def(&runtime, "ResultCode"),
        Type::Enum { base, values, .. }
            if *base == TypeId::DINT
                && values.as_slice() == [("Ready".into(), 10), ("Busy".into(), 20)]
    ));
}

#[test]
fn type_contract_type_default_is_bound_to_declared_alias() {
    let runtime = runtime(
        r#"
TYPE Speed : INT := INT#7;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let speed = type_id(&runtime, "Speed");
    let initializer = runtime
        .initializer_catalog()
        .type_default(speed)
        .expect("Speed type default");
    assert!(matches!(
        runtime.initializer_catalog().initializer(initializer),
        Some(Expr::Literal(Value::Int(7)))
    ));
}

#[test]
fn type_contract_enum_default_resolves_named_variant() {
    let runtime = runtime(
        r#"
TYPE Phase : (Idle, Running) := Running;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let phase = type_id(&runtime, "Phase");
    let initializer = runtime
        .initializer_catalog()
        .type_default(phase)
        .expect("Phase type default");
    assert!(
        matches!(
            runtime.initializer_catalog().initializer(initializer),
            Some(Expr::Literal(Value::Enum(value)))
                if value.type_name() == "Phase"
                    && value.variant_name() == "Running"
                    && value.numeric_value() == 1
        ),
        "{:?}",
        runtime.initializer_catalog().initializer(initializer)
    );
}

#[test]
fn type_contract_bounded_string_retains_character_capacity() {
    let runtime = runtime(
        r#"
TYPE Label : STRING[12];
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let target = match type_def(&runtime, "Label") {
        Type::Alias { target, .. } => *target,
        other => panic!("expected bounded STRING alias, got {other:?}"),
    };
    assert!(matches!(
        runtime.registry().get(target),
        Some(Type::String { max_len: Some(12) })
    ));
}

#[test]
fn type_contract_bounded_wstring_retains_character_capacity() {
    let runtime = runtime(
        r#"
TYPE WideLabel : WSTRING[9];
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let target = match type_def(&runtime, "WideLabel") {
        Type::Alias { target, .. } => *target,
        other => panic!("expected bounded WSTRING alias, got {other:?}"),
    };
    assert!(matches!(
        runtime.registry().get(target),
        Some(Type::WString { max_len: Some(9) })
    ));
}

#[test]
fn type_contract_reference_alias_retains_target_type() {
    let runtime = runtime(
        r#"
TYPE IntReference : REF_TO INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let target = match type_def(&runtime, "IntReference") {
        Type::Alias { target, .. } => *target,
        other => panic!("expected REF_TO alias, got {other:?}"),
    };
    assert!(matches!(
        runtime.registry().get(target),
        Some(Type::Reference {
            target: TypeId::INT
        })
    ));
}

#[test]
fn type_contract_pointer_extension_alias_retains_target_type() {
    let runtime = runtime(
        r#"
TYPE IntPointer : POINTER TO INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    let target = match type_def(&runtime, "IntPointer") {
        Type::Alias { target, .. } => *target,
        other => panic!("expected POINTER TO alias, got {other:?}"),
    };
    assert!(matches!(
        runtime.registry().get(target),
        Some(Type::Pointer {
            target: TypeId::INT
        })
    ));
}

#[test]
fn type_contract_predeclares_function_block_class_and_interface_types() {
    let runtime = runtime(
        r#"
INTERFACE IDevice
END_INTERFACE
CLASS Device
END_CLASS
FUNCTION_BLOCK Controller
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(matches!(
        type_def(&runtime, "idevice"),
        Type::Interface { name } if name == "IDevice"
    ));
    assert!(matches!(
        type_def(&runtime, "DEVICE"),
        Type::Class { name } if name == "Device"
    ));
    assert!(matches!(
        type_def(&runtime, "controller"),
        Type::FunctionBlock { name } if name == "Controller"
    ));
}

#[test]
fn type_contract_rejects_case_only_duplicate_derived_type_names() {
    let error = compile_error(
        r#"
TYPE
BatchCode : INT;
batchcode : DINT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("duplicate") && error.to_ascii_lowercase().contains("batchcode"),
        "{error}"
    );
}

#[test]
fn type_contract_rejects_cross_kind_type_name_collisions() {
    let error = compile_error(
        r#"
TYPE Device : INT;
END_TYPE
CLASS device
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("duplicate") && error.to_ascii_lowercase().contains("device"),
        "{error}"
    );
}

#[test]
fn type_contract_rejects_unknown_alias_target() {
    let error = compile_error(
        r#"
TYPE Broken : MissingType;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("E102") && error.contains("MissingType"),
        "{error}"
    );
}

#[test]
fn type_contract_rejects_unknown_structure_member_type() {
    let error = compile_error(
        r#"
TYPE Broken : STRUCT
    Field : MissingType;
END_STRUCT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("E102") && error.contains("MissingType"),
        "{error}"
    );
}

#[test]
fn type_contract_rejects_non_positive_string_capacity() {
    let error = compile_error(
        r#"
TYPE EmptyText : STRING[0];
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("E304")
            && error.to_ascii_lowercase().contains("string")
            && error.to_ascii_lowercase().contains("positive"),
        "{error}"
    );
}
