use super::*;

use std::sync::Arc;

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::types::InitializerId;

use crate::program_model::{ArgValue, CallArg};

fn field(name: &str, type_id: TypeId, default_initializer: Option<InitializerId>) -> StructField {
    StructField {
        name: name.into(),
        type_id,
        address: None,
        default_initializer,
    }
}

fn variant(
    name: &str,
    type_id: TypeId,
    default_initializer: Option<InitializerId>,
) -> UnionVariant {
    UnionVariant {
        name: name.into(),
        type_id,
        address: None,
        default_initializer,
    }
}

fn struct_value(type_name: &str, fields: &[(&str, Value)]) -> Value {
    let fields = fields
        .iter()
        .map(|(name, value)| ((*name).into(), value.clone()))
        .collect::<IndexMap<_, _>>();
    Value::Struct(Arc::new(StructValue::from_untyped_parts(
        type_name.into(),
        fields,
    )))
}

fn materialize(
    registry: &TypeRegistry,
    catalog: &InitializerCatalog,
    type_id: TypeId,
) -> Result<Value, RuntimeError> {
    default_value_for_type_id(
        &VariableStorage::new(),
        registry,
        catalog,
        &DateTimeProfile::default(),
        None,
        &StandardLibrary::new(),
        type_id,
    )
}

fn coerce(
    registry: &TypeRegistry,
    catalog: &InitializerCatalog,
    value: Value,
    type_id: TypeId,
) -> Result<Value, RuntimeError> {
    apply_aggregate_overrides(
        &VariableStorage::new(),
        registry,
        catalog,
        &DateTimeProfile::default(),
        None,
        &StandardLibrary::new(),
        value,
        type_id,
    )
}

#[test]
fn harness_initializer_contract_array_len_uses_checked_inclusive_extents() {
    assert_eq!(array_len(&[(1, 3)]), Ok(3));
    assert_eq!(array_len(&[(1, 2), (-1, 1)]), Ok(6));
    assert_eq!(array_len(&[]), Ok(1));
}

#[test]
fn harness_initializer_contract_array_len_rejects_reversed_and_overflowing_extents() {
    assert_eq!(array_len(&[(2, 1)]), Err(RuntimeError::TypeMismatch));
    assert_eq!(
        array_len(&[(i64::MIN, i64::MAX)]),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        array_len(&[(0, i64::MAX), (0, 1)]),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn harness_initializer_contract_elementary_defaults_preserve_runtime_tags() {
    let registry = TypeRegistry::new();
    let catalog = InitializerCatalog::default();
    for (type_id, expected) in [
        (TypeId::BOOL, Value::Bool(false)),
        (TypeId::SINT, Value::SInt(0)),
        (TypeId::INT, Value::Int(0)),
        (TypeId::DINT, Value::DInt(0)),
        (TypeId::LINT, Value::LInt(0)),
        (TypeId::USINT, Value::USInt(0)),
        (TypeId::UINT, Value::UInt(0)),
        (TypeId::UDINT, Value::UDInt(0)),
        (TypeId::ULINT, Value::ULInt(0)),
        (TypeId::REAL, Value::Real(0.0)),
        (TypeId::LREAL, Value::LReal(0.0)),
        (TypeId::STRING, Value::String("".into())),
        (TypeId::WSTRING, Value::WString(String::new())),
    ] {
        assert_eq!(materialize(&registry, &catalog, type_id), Ok(expected));
    }
}

#[test]
fn harness_initializer_contract_alias_default_delegates_to_target() {
    let mut registry = TypeRegistry::new();
    let alias = registry.register(
        "Counter",
        Type::Alias {
            name: "Counter".into(),
            target: TypeId::DINT,
        },
    );
    assert_eq!(
        materialize(&registry, &InitializerCatalog::default(), alias),
        Ok(Value::DInt(0))
    );
}

#[test]
fn harness_initializer_contract_any_int_default_is_explicit_null() {
    let mut registry = TypeRegistry::new();
    let any_int = registry.register("GenericCounter", Type::AnyInt);
    assert_eq!(
        materialize(&registry, &InitializerCatalog::default(), any_int),
        Ok(Value::Null)
    );
}

#[test]
fn harness_initializer_contract_unknown_type_fails_closed() {
    let registry = TypeRegistry::new();
    assert_eq!(
        materialize(&registry, &InitializerCatalog::default(), TypeId(u32::MAX)),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn harness_initializer_contract_registered_type_default_precedes_builtin_default() {
    let registry = TypeRegistry::new();
    let mut catalog = InitializerCatalog::default();
    let initializer = catalog.insert(Expr::Literal(Value::SInt(7)));
    catalog.set_type_default(TypeId::INT, initializer);
    assert_eq!(
        materialize(&registry, &catalog, TypeId::INT),
        Ok(Value::Int(7))
    );
}

#[test]
fn harness_initializer_contract_type_default_can_read_supplied_storage() {
    let registry = TypeRegistry::new();
    let mut catalog = InitializerCatalog::default();
    let initializer = catalog.insert(Expr::Name("seed".into()));
    catalog.set_type_default(TypeId::INT, initializer);
    let mut storage = VariableStorage::new();
    storage.set_global("seed", Value::Int(9));

    assert_eq!(
        default_value_for_type_id(
            &storage,
            &registry,
            &catalog,
            &DateTimeProfile::default(),
            None,
            &StandardLibrary::new(),
            TypeId::INT,
        ),
        Ok(Value::Int(9))
    );
}

#[test]
fn harness_initializer_contract_type_default_can_use_standard_library() {
    let registry = TypeRegistry::new();
    let mut catalog = InitializerCatalog::default();
    let initializer = catalog.insert(Expr::Call {
        target: Box::new(Expr::Name("ABS".into())),
        args: vec![CallArg {
            name: None,
            value: ArgValue::Expr(Expr::Literal(Value::DInt(-7))),
        }],
    });
    catalog.set_type_default(TypeId::DINT, initializer);

    assert_eq!(
        materialize(&registry, &catalog, TypeId::DINT),
        Ok(Value::DInt(7))
    );
}

#[test]
fn harness_initializer_contract_missing_type_default_record_is_error() {
    let registry = TypeRegistry::new();
    let mut catalog = InitializerCatalog::default();
    catalog.set_type_default(TypeId::INT, InitializerId(99));
    assert_eq!(
        materialize(&registry, &catalog, TypeId::INT),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn harness_initializer_contract_fixed_array_default_is_recursive_and_canonical() {
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::INT, vec![(1, 2), (-1, 0)]);
    let Value::Array(value) =
        materialize(&registry, &InitializerCatalog::default(), array).unwrap()
    else {
        panic!("expected array");
    };
    assert_eq!(value.dimensions(), &[(1, 2), (-1, 0)]);
    assert_eq!(value.elements(), vec![Value::Int(0); 4].as_slice());
}

#[test]
fn harness_initializer_contract_wildcard_array_default_retains_empty_shape() {
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::BOOL, vec![(0, i64::MAX)]);
    let Value::Array(value) =
        materialize(&registry, &InitializerCatalog::default(), array).unwrap()
    else {
        panic!("expected wildcard array");
    };
    assert_eq!(value.dimensions(), &[(0, i64::MAX)]);
    assert!(value.elements().is_empty());
}

#[test]
fn harness_initializer_contract_invalid_array_default_fails_before_publication() {
    let mut registry = TypeRegistry::new();
    let reversed = registry.register_array(TypeId::INT, vec![(2, 1)]);
    let overflowing = registry.register_array(TypeId::INT, vec![(i64::MIN, i64::MAX)]);
    for type_id in [reversed, overflowing] {
        assert_eq!(
            materialize(&registry, &InitializerCatalog::default(), type_id),
            Err(RuntimeError::TypeMismatch)
        );
    }
}

#[test]
fn harness_initializer_contract_member_initializer_precedes_member_type_default() {
    let mut registry = TypeRegistry::new();
    let mut catalog = InitializerCatalog::default();
    let type_default = catalog.insert(Expr::Literal(Value::Int(3)));
    catalog.set_type_default(TypeId::INT, type_default);
    let member_default = catalog.insert(Expr::Literal(Value::SInt(7)));
    let record = registry.register_struct(
        "Record",
        vec![
            field("explicit", TypeId::INT, Some(member_default)),
            field("implicit", TypeId::INT, None),
        ],
    );

    let Value::Struct(value) = materialize(&registry, &catalog, record).unwrap() else {
        panic!("expected struct");
    };
    assert_eq!(value.field("explicit"), Some(&Value::Int(7)));
    assert_eq!(value.field("implicit"), Some(&Value::Int(3)));
}

#[test]
fn harness_initializer_contract_missing_member_initializer_record_is_error() {
    let mut registry = TypeRegistry::new();
    let record = registry.register_struct(
        "Record",
        vec![field("value", TypeId::INT, Some(InitializerId(99)))],
    );
    assert_eq!(
        materialize(&registry, &InitializerCatalog::default(), record),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn harness_initializer_contract_struct_default_preserves_name_order_and_nested_defaults() {
    let mut registry = TypeRegistry::new();
    let inner = registry.register_struct("Inner", vec![field("flag", TypeId::BOOL, None)]);
    let outer = registry.register_struct(
        "Outer",
        vec![
            field("count", TypeId::DINT, None),
            field("inner", inner, None),
        ],
    );

    let Value::Struct(value) =
        materialize(&registry, &InitializerCatalog::default(), outer).unwrap()
    else {
        panic!("expected outer struct");
    };
    assert_eq!(value.type_name(), "Outer");
    assert_eq!(
        value
            .fields()
            .keys()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        vec!["count", "inner"]
    );
    let Some(Value::Struct(inner)) = value.field("inner") else {
        panic!("expected inner struct");
    };
    assert_eq!(inner.type_name(), "Inner");
    assert_eq!(inner.field("flag"), Some(&Value::Bool(false)));
}

#[test]
fn harness_initializer_contract_union_default_preserves_all_declared_variants() {
    let mut registry = TypeRegistry::new();
    let choice = registry.register_union(
        "Choice",
        vec![
            variant("number", TypeId::INT, None),
            variant("flag", TypeId::BOOL, None),
        ],
    );
    let Value::Struct(value) =
        materialize(&registry, &InitializerCatalog::default(), choice).unwrap()
    else {
        panic!("expected union representation");
    };
    assert_eq!(value.type_name(), "Choice");
    assert_eq!(value.field("number"), Some(&Value::Int(0)));
    assert_eq!(value.field("flag"), Some(&Value::Bool(false)));
}

#[test]
fn harness_initializer_contract_partial_array_uses_registered_element_default() {
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::INT, vec![(1, 3)]);
    let mut catalog = InitializerCatalog::default();
    let default = catalog.insert(Expr::Literal(Value::Int(9)));
    catalog.set_type_default(TypeId::INT, default);
    let input = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::SInt(1)],
        vec![(0, 0)],
    )));

    let Value::Array(value) = coerce(&registry, &catalog, input, array).unwrap() else {
        panic!("expected array");
    };
    assert_eq!(value.dimensions(), &[(1, 3)]);
    assert_eq!(
        value.elements(),
        &[Value::Int(1), Value::Int(9), Value::Int(9)]
    );
}

#[test]
fn harness_initializer_contract_array_override_rejects_nonarray_and_bad_element_but_ignores_excess()
{
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::INT, vec![(1, 2)]);
    let catalog = InitializerCatalog::default();
    assert_eq!(
        coerce(&registry, &catalog, Value::Int(1), array),
        Err(RuntimeError::TypeMismatch)
    );
    let excess = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(1), Value::Int(2), Value::Int(3)],
        vec![(0, 2)],
    )));
    let Value::Array(excess) = coerce(&registry, &catalog, excess, array).unwrap() else {
        panic!("expected array");
    };
    assert_eq!(excess.elements(), &[Value::Int(1), Value::Int(2)]);
    let bad = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Bool(true)],
        vec![(0, 0)],
    )));
    assert_eq!(
        coerce(&registry, &catalog, bad, array),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn harness_initializer_contract_struct_override_is_named_case_insensitive_and_recursive() {
    let mut registry = TypeRegistry::new();
    let record = registry.register_struct(
        "Record",
        vec![
            field("Count", TypeId::DINT, None),
            field("Enabled", TypeId::BOOL, None),
        ],
    );
    let input = struct_value("input", &[("count", Value::SInt(7))]);

    let Value::Struct(value) =
        coerce(&registry, &InitializerCatalog::default(), input, record).unwrap()
    else {
        panic!("expected struct");
    };
    assert_eq!(value.type_name(), "Record");
    assert_eq!(value.field("Count"), Some(&Value::DInt(7)));
    assert_eq!(value.field("Enabled"), Some(&Value::Bool(false)));
}

#[test]
fn harness_initializer_contract_struct_override_rejects_unknown_and_wrong_shape() {
    let mut registry = TypeRegistry::new();
    let record = registry.register_struct("Record", vec![field("Count", TypeId::DINT, None)]);
    let catalog = InitializerCatalog::default();
    assert_eq!(
        coerce(
            &registry,
            &catalog,
            struct_value("input", &[("missing", Value::DInt(1))]),
            record,
        ),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        coerce(&registry, &catalog, Value::DInt(1), record),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn harness_initializer_contract_union_override_is_named_case_insensitive_and_recursive() {
    let mut registry = TypeRegistry::new();
    let choice = registry.register_union(
        "Choice",
        vec![
            variant("Number", TypeId::INT, None),
            variant("Flag", TypeId::BOOL, None),
        ],
    );
    let input = struct_value("input", &[("flag", Value::Bool(true))]);

    let Value::Struct(value) =
        coerce(&registry, &InitializerCatalog::default(), input, choice).unwrap()
    else {
        panic!("expected union representation");
    };
    assert_eq!(value.type_name(), "Choice");
    assert_eq!(value.field("Number"), Some(&Value::Int(0)));
    assert_eq!(value.field("Flag"), Some(&Value::Bool(true)));
}

#[test]
fn harness_initializer_contract_union_override_rejects_unknown_and_wrong_shape() {
    let mut registry = TypeRegistry::new();
    let choice = registry.register_union("Choice", vec![variant("Number", TypeId::INT, None)]);
    let catalog = InitializerCatalog::default();
    assert_eq!(
        coerce(
            &registry,
            &catalog,
            struct_value("input", &[("missing", Value::Int(1))]),
            choice,
        ),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        coerce(&registry, &catalog, Value::Int(1), choice),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn harness_initializer_contract_evaluate_literal_then_applies_aggregate_override() {
    let mut registry = TypeRegistry::new();
    let record = registry.register_struct(
        "Record",
        vec![
            field("Count", TypeId::DINT, None),
            field("Enabled", TypeId::BOOL, None),
        ],
    );
    let expression = Expr::Literal(struct_value("input", &[("count", Value::SInt(7))]));

    let Value::Struct(value) = evaluate_initializer(
        &VariableStorage::new(),
        &registry,
        &InitializerCatalog::default(),
        &DateTimeProfile::default(),
        None,
        &StandardLibrary::new(),
        &expression,
        record,
    )
    .unwrap() else {
        panic!("expected evaluated struct");
    };
    assert_eq!(value.field("Count"), Some(&Value::DInt(7)));
    assert_eq!(value.field("Enabled"), Some(&Value::Bool(false)));
}

#[test]
fn harness_initializer_contract_depth_boundary_is_inclusive_and_then_closed() {
    let registry = TypeRegistry::new();
    let storage = VariableStorage::new();
    let catalog = InitializerCatalog::default();
    let profile = DateTimeProfile::default();
    let stdlib = StandardLibrary::new();
    let ctx = InitContext {
        storage: &storage,
        registry: &registry,
        catalog: &catalog,
        profile: &profile,
        current_instance: None,
        stdlib: &stdlib,
    };

    assert_eq!(
        materialize_default_value(&ctx, TypeId::INT, MAX_INITIALIZER_DEPTH),
        Ok(Value::Int(0))
    );
    assert_eq!(
        materialize_default_value(&ctx, TypeId::INT, MAX_INITIALIZER_DEPTH + 1),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        coerce_initializer_value(&ctx, Value::SInt(1), TypeId::INT, MAX_INITIALIZER_DEPTH),
        Ok(Value::Int(1))
    );
    assert_eq!(
        coerce_initializer_value(&ctx, Value::SInt(1), TypeId::INT, MAX_INITIALIZER_DEPTH + 1),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn harness_initializer_contract_alias_cycle_terminates_at_depth_guard() {
    let mut registry = TypeRegistry::new();
    let cycle = registry.reserve("Cycle");
    registry.replace(
        cycle,
        Type::Alias {
            name: "Cycle".into(),
            target: cycle,
        },
    );
    assert_eq!(
        materialize(&registry, &InitializerCatalog::default(), cycle),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        coerce(
            &registry,
            &InitializerCatalog::default(),
            Value::Int(1),
            cycle,
        ),
        Err(RuntimeError::TypeMismatch)
    );
}
