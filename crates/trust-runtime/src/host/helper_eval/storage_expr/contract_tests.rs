use super::*;

use std::sync::Arc;

use crate::program_model::LValue;
use trust_hir::TypeId;

fn profile() -> DateTimeProfile {
    DateTimeProfile::default()
}

fn registry() -> TypeRegistry {
    TypeRegistry::new()
}

fn literal(value: Value) -> Expr {
    Expr::Literal(value)
}

fn positional(value: Expr) -> CallArg {
    CallArg {
        name: None,
        value: ArgValue::Expr(value),
    }
}

fn named(name: &str, value: Expr) -> CallArg {
    CallArg {
        name: Some(name.into()),
        value: ArgValue::Expr(value),
    }
}

fn named_target(name: &str, target: LValue) -> CallArg {
    CallArg {
        name: Some(name.into()),
        value: ArgValue::Target(target),
    }
}

fn call(target: Expr, args: Vec<CallArg>) -> Expr {
    Expr::Call {
        target: Box::new(target),
        args,
    }
}

fn divide_by_zero() -> Expr {
    Expr::Binary {
        op: BinaryOp::Div,
        left: Box::new(literal(Value::Int(1))),
        right: Box::new(literal(Value::Int(0))),
    }
}

fn array(elements: Vec<Value>, dimensions: Vec<(i64, i64)>) -> Value {
    Value::Array(Box::new(
        ArrayValue::from_untyped_parts(elements, dimensions).expect("valid test array"),
    ))
}

fn structure(type_name: &str, fields: &[(&str, Value)]) -> Value {
    Value::Struct(Arc::new(StructValue::from_untyped_parts(
        type_name.into(),
        fields
            .iter()
            .map(|(name, value)| (SmolStr::new(*name), value.clone()))
            .collect(),
    )))
}

fn evaluate(
    storage: &VariableStorage,
    current_instance: Option<InstanceId>,
    expression: &Expr,
) -> Result<Value, RuntimeError> {
    eval_storage_expr(
        storage,
        &registry(),
        &profile(),
        current_instance,
        expression,
    )
}

fn evaluate_with_stdlib(
    storage: &VariableStorage,
    current_instance: Option<InstanceId>,
    expression: &Expr,
) -> Result<Value, RuntimeError> {
    let stdlib = StandardLibrary::new();
    eval_storage_expr_with_stdlib(
        storage,
        &registry(),
        &profile(),
        current_instance,
        Some(&stdlib),
        expression,
    )
}

#[test]
fn name_lookup_uses_local_instance_global_then_retain_precedence() {
    let mut storage = VariableStorage::new();
    storage.set_retain("local_wins", Value::Int(1));
    storage.set_global("local_wins", Value::Int(2));
    storage.set_retain("instance_wins", Value::Int(3));
    storage.set_global("instance_wins", Value::Int(4));
    storage.set_retain("global_wins", Value::Int(5));
    storage.set_global("global_wins", Value::Int(6));
    storage.set_retain("retain_only", Value::Int(7));

    let instance = storage.create_instance("CHILD");
    assert!(storage.set_instance_var(instance, "local_wins", Value::Int(8)));
    assert!(storage.set_instance_var(instance, "instance_wins", Value::Int(9)));
    storage.push_frame("METHOD");
    assert!(storage.set_local("local_wins", Value::Int(10)));

    for (name, expected) in [
        ("local_wins", Value::Int(10)),
        ("instance_wins", Value::Int(9)),
        ("global_wins", Value::Int(6)),
        ("retain_only", Value::Int(7)),
    ] {
        assert_eq!(
            evaluate(&storage, Some(instance), &Expr::Name(name.into())),
            Ok(expected),
            "lookup precedence for {name}"
        );
    }
}

#[test]
fn inherited_instance_field_precedes_global_and_retain_names() {
    let mut storage = VariableStorage::new();
    let base = storage.create_instance("BASE");
    let child = storage.create_instance("CHILD");
    storage.get_instance_mut(child).expect("child").parent = Some(base);
    assert!(storage.set_instance_var(base, "value", Value::DInt(11)));
    storage.set_global("value", Value::DInt(22));
    storage.set_retain("value", Value::DInt(33));

    assert_eq!(
        evaluate(&storage, Some(child), &Expr::Name("value".into())),
        Ok(Value::DInt(11))
    );
}

#[test]
fn missing_name_returns_exact_undefined_variable() {
    assert_eq!(
        evaluate(&VariableStorage::new(), None, &Expr::Name("missing".into())),
        Err(RuntimeError::UndefinedVariable("missing".into()))
    );
}

#[test]
fn this_and_super_return_exact_current_and_parent_instances() {
    let mut storage = VariableStorage::new();
    let base = storage.create_instance("BASE");
    let child = storage.create_instance("CHILD");
    storage.get_instance_mut(child).expect("child").parent = Some(base);

    assert_eq!(
        evaluate(&storage, Some(child), &Expr::This),
        Ok(Value::Instance(child))
    );
    assert_eq!(
        evaluate(&storage, Some(child), &Expr::Super),
        Ok(Value::Instance(base))
    );
}

#[test]
fn this_and_super_fail_closed_for_missing_or_stale_context() {
    let storage = VariableStorage::new();
    assert_eq!(
        evaluate(&storage, None, &Expr::This),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        evaluate(&storage, None, &Expr::Super),
        Err(RuntimeError::TypeMismatch)
    );

    let mut with_root = VariableStorage::new();
    let root = with_root.create_instance("ROOT");
    assert_eq!(
        evaluate(&with_root, Some(root), &Expr::Super),
        Err(RuntimeError::TypeMismatch)
    );

    with_root.reset_runtime_values(false);
    assert_eq!(
        evaluate(&with_root, Some(root), &Expr::Super),
        Err(RuntimeError::NullReference)
    );
}

#[test]
fn exact_qualified_storage_name_precedes_aggregate_field_fallback() {
    let mut storage = VariableStorage::new();
    storage.set_global("Config", structure("Config", &[("Value", Value::DInt(10))]));
    storage.set_global("Config.Value", Value::DInt(20));
    let expression = Expr::Field {
        target: Box::new(Expr::Name("Config".into())),
        field: "Value".into(),
    };

    assert_eq!(evaluate(&storage, None, &expression), Ok(Value::DInt(20)));
}

#[test]
fn field_chain_falls_back_to_structure_when_qualified_name_is_absent() {
    let mut storage = VariableStorage::new();
    storage.set_global("Config", structure("Config", &[("Value", Value::DInt(10))]));
    let expression = Expr::Field {
        target: Box::new(Expr::Name("Config".into())),
        field: "Value".into(),
    };

    assert_eq!(evaluate(&storage, None, &expression), Ok(Value::DInt(10)));
}

#[test]
fn instance_field_read_is_recursive_and_missing_field_is_explicit() {
    let mut storage = VariableStorage::new();
    let base = storage.create_instance("BASE");
    let child = storage.create_instance("CHILD");
    storage.get_instance_mut(child).expect("child").parent = Some(base);
    assert!(storage.set_instance_var(base, "inherited", Value::Int(4)));

    assert_eq!(
        read_field(&storage, Value::Instance(child), &"inherited".into()),
        Ok(Value::Int(4))
    );
    assert_eq!(
        read_field(&storage, Value::Instance(child), &"missing".into()),
        Err(RuntimeError::UndefinedField("missing".into()))
    );
}

#[test]
fn partial_field_access_preserves_value_and_bounds_errors() {
    assert_eq!(
        read_field(
            &VariableStorage::new(),
            Value::Byte(0b0000_0010),
            &"%X1".into()
        ),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        read_field(&VariableStorage::new(), Value::Byte(0), &"%X8".into()),
        Err(RuntimeError::IndexOutOfBounds {
            index: 8,
            lower: 0,
            upper: 7,
        })
    );
    assert_eq!(
        read_field(&VariableStorage::new(), Value::Bool(false), &"%B0".into()),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn structure_initializer_preserves_order_and_values() {
    let expression = Expr::StructInitializer(vec![
        ("Second".into(), literal(Value::Int(2))),
        ("First".into(), literal(Value::Int(1))),
    ]);
    let Value::Struct(value) =
        evaluate(&VariableStorage::new(), None, &expression).expect("structure initializer")
    else {
        panic!("expected structure");
    };

    assert_eq!(
        value
            .fields()
            .keys()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        ["Second", "First"]
    );
    assert_eq!(value.field("Second"), Some(&Value::Int(2)));
    assert_eq!(value.field("First"), Some(&Value::Int(1)));
}

#[test]
fn structure_initializer_rejects_case_insensitive_duplicate_fields() {
    let expression = Expr::StructInitializer(vec![
        ("Value".into(), literal(Value::Int(1))),
        ("vAlUe".into(), literal(Value::Int(2))),
    ]);

    assert_eq!(
        evaluate(&VariableStorage::new(), None, &expression),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn array_repeat_expands_source_order_and_zero_group_contributes_nothing() {
    let repeated = call(
        literal(Value::UInt(2)),
        vec![
            positional(literal(Value::Int(1))),
            positional(literal(Value::Int(2))),
        ],
    );
    let zero = call(
        literal(Value::Int(0)),
        vec![positional(literal(Value::Int(99)))],
    );
    let expression = Expr::ArrayInitializer(vec![zero, repeated, literal(Value::Int(3))]);

    let Value::Array(value) =
        evaluate(&VariableStorage::new(), None, &expression).expect("array initializer")
    else {
        panic!("expected array");
    };

    assert_eq!(value.dimensions(), &[(1, 5)]);
    assert_eq!(
        value.elements(),
        &[
            Value::Int(1),
            Value::Int(2),
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
        ]
    );
}

#[test]
fn invalid_repeat_counts_and_named_repeat_args_fail_closed() {
    for expression in [
        call(
            literal(Value::LInt(-1)),
            vec![positional(literal(Value::Int(1)))],
        ),
        call(
            literal(Value::ULInt(u64::MAX)),
            vec![positional(literal(Value::Int(1)))],
        ),
        call(
            literal(Value::Int(1)),
            vec![named("value", literal(Value::Int(1)))],
        ),
    ] {
        assert_eq!(
            evaluate(
                &VariableStorage::new(),
                None,
                &Expr::ArrayInitializer(vec![expression])
            ),
            Err(RuntimeError::TypeMismatch)
        );
    }
}

#[test]
fn boolean_and_or_short_circuit_rhs_errors() {
    for (op, left, expected) in [
        (BinaryOp::And, Value::Bool(false), Value::Bool(false)),
        (BinaryOp::AndThen, Value::Bool(false), Value::Bool(false)),
        (BinaryOp::Or, Value::Bool(true), Value::Bool(true)),
        (BinaryOp::OrElse, Value::Bool(true), Value::Bool(true)),
    ] {
        let expression = Expr::Binary {
            op,
            left: Box::new(literal(left)),
            right: Box::new(divide_by_zero()),
        };
        assert_eq!(
            evaluate(&VariableStorage::new(), None, &expression),
            Ok(expected)
        );
    }
}

#[test]
fn non_short_circuited_boolean_paths_evaluate_rhs() {
    for (op, left) in [
        (BinaryOp::And, Value::Bool(true)),
        (BinaryOp::Or, Value::Bool(false)),
        (BinaryOp::Xor, Value::Bool(false)),
    ] {
        let expression = Expr::Binary {
            op,
            left: Box::new(literal(left)),
            right: Box::new(divide_by_zero()),
        };
        assert_eq!(
            evaluate(&VariableStorage::new(), None, &expression),
            Err(RuntimeError::DivisionByZero)
        );
    }
}

#[test]
fn bit_string_and_does_not_use_boolean_short_circuit() {
    let expression = Expr::Binary {
        op: BinaryOp::And,
        left: Box::new(literal(Value::Byte(0))),
        right: Box::new(divide_by_zero()),
    };

    assert_eq!(
        evaluate(&VariableStorage::new(), None, &expression),
        Err(RuntimeError::DivisionByZero)
    );
}

#[test]
fn multidimensional_array_index_honors_inclusive_lower_bounds() {
    let expression = Expr::Index {
        target: Box::new(literal(array(
            vec![
                Value::Int(10),
                Value::Int(11),
                Value::Int(12),
                Value::Int(13),
            ],
            vec![(5, 6), (-1, 0)],
        ))),
        indices: vec![literal(Value::Int(6)), literal(Value::SInt(-1))],
    };

    assert_eq!(
        evaluate(&VariableStorage::new(), None, &expression),
        Ok(Value::Int(12))
    );
}

#[test]
fn array_index_reports_arity_type_and_bounds_partitions() {
    let target = array(vec![Value::Int(10), Value::Int(11)], vec![(5, 6)]);

    let cases = [
        (Vec::new(), RuntimeError::TypeMismatch),
        (vec![Value::Bool(true)], RuntimeError::TypeMismatch),
        (
            vec![Value::Int(7)],
            RuntimeError::IndexOutOfBounds {
                index: 7,
                lower: 5,
                upper: 6,
            },
        ),
    ];
    for (indices, expected) in cases {
        let expression = Expr::Index {
            target: Box::new(literal(target.clone())),
            indices: indices.into_iter().map(literal).collect(),
        };
        assert_eq!(
            evaluate(&VariableStorage::new(), None, &expression),
            Err(expected)
        );
    }
}

#[test]
fn string_and_wstring_indices_are_one_based_unicode_elements() {
    for (target, expected) in [
        (Value::String("ÄB".into()), Value::Char(0x00C4)),
        (Value::WString("ÄB".into()), Value::WChar(0x00C4)),
    ] {
        let expression = Expr::Index {
            target: Box::new(literal(target)),
            indices: vec![literal(Value::Int(1))],
        };
        assert_eq!(
            evaluate(&VariableStorage::new(), None, &expression),
            Ok(expected)
        );
    }
}

#[test]
fn string_index_reports_wrong_arity_type_and_bounds() {
    for (indices, expected) in [
        (Vec::new(), RuntimeError::TypeMismatch),
        (vec![Value::Bool(true)], RuntimeError::TypeMismatch),
        (
            vec![Value::Int(0)],
            RuntimeError::IndexOutOfBounds {
                index: 0,
                lower: 1,
                upper: i64::MAX,
            },
        ),
    ] {
        let expression = Expr::Index {
            target: Box::new(literal(Value::String("AB".into()))),
            indices: indices.into_iter().map(literal).collect(),
        };
        assert_eq!(
            evaluate(&VariableStorage::new(), None, &expression),
            Err(expected)
        );
    }
}

#[test]
fn ref_name_uses_local_instance_then_global_but_not_retain_storage() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::Int(1));
    let instance = storage.create_instance("OWNER");
    assert!(storage.set_instance_var(instance, "x", Value::Int(2)));
    storage.push_frame("METHOD");
    assert!(storage.set_local("x", Value::Int(3)));

    let dereference = Expr::Deref(Box::new(Expr::Ref(LValue::Name("x".into()))));
    assert_eq!(
        evaluate(&storage, Some(instance), &dereference),
        Ok(Value::Int(3))
    );

    let mut retained_only = VariableStorage::new();
    retained_only.set_retain("r", Value::Int(4));
    assert_eq!(
        evaluate(&retained_only, None, &Expr::Ref(LValue::Name("r".into()))),
        Err(RuntimeError::UndefinedVariable("r".into()))
    );
}

#[test]
fn ref_index_validates_path_and_dereferences_selected_element() {
    let mut storage = VariableStorage::new();
    storage.set_global(
        "arr",
        array(vec![Value::Int(10), Value::Int(20)], vec![(5, 6)]),
    );
    let reference = Expr::Ref(LValue::Index {
        target: Box::new(LValue::Name("arr".into())),
        indices: vec![literal(Value::Int(6))],
    });

    assert_eq!(
        evaluate(&storage, None, &Expr::Deref(Box::new(reference.clone()))),
        Ok(Value::Int(20))
    );

    let invalid = Expr::Ref(LValue::Index {
        target: Box::new(LValue::Name("arr".into())),
        indices: vec![literal(Value::Int(7))],
    });
    assert_eq!(
        evaluate(&storage, None, &invalid),
        Err(RuntimeError::IndexOutOfBounds {
            index: 7,
            lower: 5,
            upper: 6,
        })
    );
}

#[test]
fn dereference_distinguishes_valid_empty_nonreference_and_stale_values() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::DInt(5));
    let reference = storage.ref_for_global("x").expect("global reference");

    assert_eq!(
        evaluate(
            &storage,
            None,
            &Expr::Deref(Box::new(literal(Value::Reference(Some(reference.clone())))))
        ),
        Ok(Value::DInt(5))
    );
    assert_eq!(
        evaluate(
            &storage,
            None,
            &Expr::Deref(Box::new(literal(Value::Reference(None))))
        ),
        Err(RuntimeError::NullReference)
    );
    assert_eq!(
        evaluate(
            &storage,
            None,
            &Expr::Deref(Box::new(literal(Value::DInt(5))))
        ),
        Err(RuntimeError::TypeMismatch)
    );

    storage.reset_runtime_values(false);
    assert_eq!(
        evaluate(
            &storage,
            None,
            &Expr::Deref(Box::new(literal(Value::Reference(Some(reference)))))
        ),
        Err(RuntimeError::NullReference)
    );
}

#[test]
fn sizeof_uses_registry_and_maps_unknown_or_unsized_types() {
    let storage = VariableStorage::new();
    assert_eq!(
        evaluate(
            &storage,
            None,
            &Expr::SizeOf(SizeOfTarget::Type(TypeId::INT))
        ),
        Ok(Value::DInt(2))
    );
    for type_id in [TypeId(900_201), TypeId::STRING] {
        assert_eq!(
            evaluate(&storage, None, &Expr::SizeOf(SizeOfTarget::Type(type_id))),
            Err(RuntimeError::TypeMismatch)
        );
    }
}

#[test]
fn calls_require_explicit_stdlib_capability_and_valid_target_name() {
    let storage = VariableStorage::new();
    let expression = call(
        Expr::Name("ABS".into()),
        vec![positional(literal(Value::DInt(-4)))],
    );

    assert_eq!(
        evaluate(&storage, None, &expression),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        evaluate_with_stdlib(&storage, None, &expression),
        Ok(Value::DInt(4))
    );

    let invalid_target = call(
        literal(Value::DInt(1)),
        vec![positional(literal(Value::DInt(2)))],
    );
    assert_eq!(
        evaluate_with_stdlib(&storage, None, &invalid_target),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn unknown_stdlib_function_preserves_qualified_target_name() {
    let expression = call(
        Expr::Field {
            target: Box::new(Expr::Name("Vendor".into())),
            field: "Missing".into(),
        },
        vec![],
    );

    assert_eq!(
        evaluate_with_stdlib(&VariableStorage::new(), None, &expression),
        Err(RuntimeError::UndefinedFunction("Vendor.Missing".into()))
    );
}

#[test]
fn call_target_name_accepts_only_name_and_field_only_chains() {
    let qualified = Expr::Field {
        target: Box::new(Expr::Field {
            target: Box::new(Expr::Name("Vendor".into())),
            field: "Math".into(),
        }),
        field: "Fn".into(),
    };
    assert_eq!(call_target_name(&qualified), Some("Vendor.Math.Fn".into()));
    assert_eq!(call_target_name(&literal(Value::Int(1))), None);
    assert_eq!(
        call_target_name(&Expr::Index {
            target: Box::new(Expr::Name("Fns".into())),
            indices: vec![literal(Value::Int(1))],
        }),
        None
    );
}

#[test]
fn fixed_named_args_are_case_insensitive_and_reordered_to_signature() {
    let expression = call(
        Expr::Name("LIMIT".into()),
        vec![
            named("mx", literal(Value::Int(10))),
            named("in", literal(Value::Int(7))),
            named("mn", literal(Value::Int(0))),
        ],
    );

    assert_eq!(
        evaluate_with_stdlib(&VariableStorage::new(), None, &expression),
        Ok(Value::Int(7))
    );
}

#[test]
fn fixed_named_args_reject_mixed_duplicate_unknown_and_missing_names() {
    let cases = [
        (
            vec![
                named("MN", literal(Value::Int(0))),
                positional(literal(Value::Int(5))),
                named("MX", literal(Value::Int(10))),
            ],
            RuntimeError::InvalidArgumentName("<unnamed>".into()),
        ),
        (
            vec![
                named("MN", literal(Value::Int(0))),
                named("mn", literal(Value::Int(1))),
                named("IN", literal(Value::Int(5))),
            ],
            RuntimeError::InvalidArgumentName("mn".into()),
        ),
        (
            vec![
                named("MN", literal(Value::Int(0))),
                named("IN", literal(Value::Int(5))),
                named("TOP", literal(Value::Int(10))),
            ],
            RuntimeError::InvalidArgumentName("TOP".into()),
        ),
        (
            vec![
                named("MN", literal(Value::Int(0))),
                named("MX", literal(Value::Int(10))),
            ],
            RuntimeError::InvalidArgumentCount {
                expected: 3,
                got: 2,
            },
        ),
    ];

    for (args, expected) in cases {
        let expression = call(Expr::Name("LIMIT".into()), args);
        assert_eq!(
            evaluate_with_stdlib(&VariableStorage::new(), None, &expression),
            Err(expected)
        );
    }
}

#[test]
fn variadic_named_args_are_reordered_and_require_contiguous_sequence() {
    let valid = call(
        Expr::Name("MAX".into()),
        vec![
            named("IN2", literal(Value::Int(9))),
            named("in1", literal(Value::Int(4))),
        ],
    );
    assert_eq!(
        evaluate_with_stdlib(&VariableStorage::new(), None, &valid),
        Ok(Value::Int(9))
    );

    for args in [
        vec![
            named("IN1", literal(Value::Int(4))),
            named("IN3", literal(Value::Int(9))),
        ],
        vec![named("IN1", literal(Value::Int(4)))],
        vec![
            named("IN1", literal(Value::Int(4))),
            named("in1", literal(Value::Int(9))),
        ],
        vec![
            named("IN0", literal(Value::Int(4))),
            named("IN1", literal(Value::Int(9))),
        ],
    ] {
        let invalid = call(Expr::Name("MAX".into()), args);
        assert!(
            evaluate_with_stdlib(&VariableStorage::new(), None, &invalid).is_err(),
            "invalid variadic name set must fail"
        );
    }
}

#[test]
fn variadic_named_args_with_fixed_prefix_bind_all_registered_slots() {
    let expression = call(
        Expr::Name("MUX".into()),
        vec![
            named("IN1", literal(Value::Int(20))),
            named("k", literal(Value::Int(1))),
            named("IN0", literal(Value::Int(10))),
        ],
    );

    assert_eq!(
        evaluate_with_stdlib(&VariableStorage::new(), None, &expression),
        Ok(Value::Int(20))
    );
}

#[test]
fn target_argument_reads_existing_lvalue_without_writing_it() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::DInt(-8));
    let expression = call(
        Expr::Name("ABS".into()),
        vec![named_target("IN", LValue::Name("x".into()))],
    );

    assert_eq!(
        evaluate_with_stdlib(&storage, None, &expression),
        Ok(Value::DInt(8))
    );
    assert_eq!(storage.get_global("x"), Some(&Value::DInt(-8)));
}

#[test]
fn conversion_calls_accept_named_input_and_reject_unknown_input_name() {
    let valid = call(
        Expr::Name("INT_TO_DINT".into()),
        vec![named("in", literal(Value::Int(12)))],
    );
    assert_eq!(
        evaluate_with_stdlib(&VariableStorage::new(), None, &valid),
        Ok(Value::DInt(12))
    );

    let invalid = call(
        Expr::Name("INT_TO_DINT".into()),
        vec![named("VALUE", literal(Value::Int(12)))],
    );
    assert_eq!(
        evaluate_with_stdlib(&VariableStorage::new(), None, &invalid),
        Err(RuntimeError::InvalidArgumentName("VALUE".into()))
    );
}

#[test]
fn integer_index_normalization_accepts_all_supported_tags() {
    let cases = [
        (Value::SInt(-1), -1),
        (Value::Int(-2), -2),
        (Value::DInt(-3), -3),
        (Value::LInt(-4), -4),
        (Value::USInt(1), 1),
        (Value::UInt(2), 2),
        (Value::UDInt(3), 3),
        (Value::ULInt(4), 4),
        (Value::Byte(5), 5),
        (Value::Word(6), 6),
        (Value::DWord(7), 7),
        (Value::LWord(8), 8),
    ];
    for (value, expected) in cases {
        assert_eq!(index_to_i64(value), Ok(expected));
    }
    assert_eq!(
        index_to_i64(Value::ULInt(u64::MAX)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        index_to_i64(Value::LWord(u64::MAX)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        index_to_i64(Value::Bool(true)),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn helper_error_mappings_preserve_bounds_type_and_overflow_classes() {
    assert_eq!(
        partial_access_error_to_runtime(PartialAccessError::IndexOutOfBounds {
            index: 8,
            lower: 0,
            upper: 7,
        }),
        RuntimeError::IndexOutOfBounds {
            index: 8,
            lower: 0,
            upper: 7,
        }
    );
    assert_eq!(
        partial_access_error_to_runtime(PartialAccessError::TypeMismatch),
        RuntimeError::TypeMismatch
    );
    assert_eq!(
        size_error_to_runtime(SizeOfError::UnknownType),
        RuntimeError::TypeMismatch
    );
    assert_eq!(
        size_error_to_runtime(SizeOfError::UnsupportedType),
        RuntimeError::TypeMismatch
    );
    assert_eq!(
        size_error_to_runtime(SizeOfError::Overflow),
        RuntimeError::Overflow
    );
}
