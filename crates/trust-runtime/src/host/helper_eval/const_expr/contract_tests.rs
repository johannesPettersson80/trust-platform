use super::*;

use std::cell::{Cell, RefCell};

use crate::program_model::ops::{BinaryOp, UnaryOp};
use crate::program_model::{ArgValue, CallArg, LValue, SizeOfTarget};
use trust_hir::TypeId;

fn positional(expr: Expr) -> CallArg {
    CallArg {
        name: None,
        value: ArgValue::Expr(expr),
    }
}

fn repeat(count: Value, values: Vec<Expr>) -> Expr {
    Expr::Call {
        target: Box::new(Expr::Literal(count)),
        args: values.into_iter().map(positional).collect(),
    }
}

fn assert_unsupported(result: Result<Value, ConstExprError>) {
    assert!(matches!(result, Err(ConstExprError::UnsupportedExpr)));
}

fn assert_runtime_error(result: Result<Value, ConstExprError>, expected: RuntimeError) {
    match result {
        Err(ConstExprError::Runtime(actual)) => assert_eq!(actual, expected),
        other => panic!("expected runtime error {expected:?}, got {other:?}"),
    }
}

#[test]
fn literal_preserves_its_runtime_tag_and_value() {
    let result = eval_const_expr(
        &Expr::Literal(Value::UDInt(4_000_000_000)),
        &DateTimeProfile::default(),
    )
    .expect("literal is constant");

    assert_eq!(result, Value::UDInt(4_000_000_000));
}

#[test]
fn unary_and_binary_errors_are_not_replaced_with_values() {
    let division = Expr::Binary {
        op: BinaryOp::Div,
        left: Box::new(Expr::Literal(Value::Int(1))),
        right: Box::new(Expr::Literal(Value::Int(0))),
    };
    assert_runtime_error(
        eval_const_expr(&division, &DateTimeProfile::default()),
        RuntimeError::DivisionByZero,
    );

    let invalid_unary = Expr::Unary {
        op: UnaryOp::Neg,
        expr: Box::new(Expr::Literal(Value::Bool(true))),
    };
    assert_runtime_error(
        eval_const_expr(&invalid_unary, &DateTimeProfile::default()),
        RuntimeError::TypeMismatch,
    );
}

#[test]
fn resolver_receives_one_exact_qualified_constant_name() {
    let expression = Expr::Field {
        target: Box::new(Expr::Field {
            target: Box::new(Expr::Name("Constants".into())),
            field: "Limits".into(),
        }),
        field: "High".into(),
    };
    let observed = RefCell::new(Vec::new());

    let value = eval_const_expr_with_resolver(&expression, &DateTimeProfile::default(), &|name| {
        observed.borrow_mut().push(name.to_owned());
        (name == "Constants.Limits.High").then_some(Value::DInt(77))
    })
    .expect("qualified constant should resolve");

    assert_eq!(value, Value::DInt(77));
    assert_eq!(observed.into_inner(), ["Constants.Limits.High"]);
}

#[test]
fn unresolved_qualified_name_does_not_fall_through_to_field_evaluation() {
    let expression = Expr::Field {
        target: Box::new(Expr::Name("Constants".into())),
        field: "Missing".into(),
    };

    assert_unsupported(eval_const_expr_with_resolver(
        &expression,
        &DateTimeProfile::default(),
        &|_| None,
    ));
}

#[test]
fn indexed_and_dereferenced_paths_are_not_constant_names() {
    let indexed = Expr::Index {
        target: Box::new(Expr::Name("Constants".into())),
        indices: vec![Expr::Literal(Value::Int(1))],
    };
    let dereferenced = Expr::Deref(Box::new(Expr::Name("ConstantRef".into())));

    assert_eq!(qualified_const_name(&indexed), None);
    assert_eq!(qualified_const_name(&dereferenced), None);
    assert_unsupported(eval_const_expr(&indexed, &DateTimeProfile::default()));
    assert_unsupported(eval_const_expr(&dereferenced, &DateTimeProfile::default()));
}

#[test]
fn runtime_only_expression_forms_are_rejected_as_constants() {
    for expression in [
        Expr::This,
        Expr::Super,
        Expr::Ref(LValue::Name("x".into())),
        Expr::StructInitializer(vec![("x".into(), Expr::Literal(Value::Int(1)))]),
    ] {
        assert_unsupported(eval_const_expr(&expression, &DateTimeProfile::default()));
    }
}

#[test]
fn sizeof_known_elementary_type_returns_dint_bytes() {
    let registry = TypeRegistry::new();
    let value = eval_const_expr_with_resolver_and_registry(
        &Expr::SizeOf(SizeOfTarget::Type(TypeId::DINT)),
        &DateTimeProfile::default(),
        &registry,
        &|_| None,
    )
    .expect("DINT size is known");

    assert_eq!(value, Value::DInt(4));
}

#[test]
fn sizeof_unknown_and_unsized_types_map_to_runtime_type_mismatch() {
    let registry = TypeRegistry::new();

    for type_id in [TypeId(900_101), TypeId::STRING] {
        assert_runtime_error(
            eval_const_expr_with_resolver_and_registry(
                &Expr::SizeOf(SizeOfTarget::Type(type_id)),
                &DateTimeProfile::default(),
                &registry,
                &|_| None,
            ),
            RuntimeError::TypeMismatch,
        );
    }
}

#[test]
fn ordinary_array_initializer_preserves_source_order_and_one_based_shape() {
    let expression = Expr::ArrayInitializer(vec![
        Expr::Literal(Value::Int(4)),
        Expr::Literal(Value::Int(2)),
        Expr::Literal(Value::Int(9)),
    ]);

    let Value::Array(array) =
        eval_const_expr(&expression, &DateTimeProfile::default()).expect("array constant")
    else {
        panic!("expected array");
    };

    assert_eq!(array.dimensions(), &[(1, 3)]);
    assert_eq!(
        array.elements(),
        &[Value::Int(4), Value::Int(2), Value::Int(9)]
    );
}

#[test]
fn array_repeat_re_evaluates_each_group_in_source_order() {
    let expression = Expr::ArrayInitializer(vec![
        repeat(
            Value::USInt(2),
            vec![Expr::Name("First".into()), Expr::Name("Second".into())],
        ),
        Expr::Name("Tail".into()),
    ]);

    let Value::Array(array) = eval_const_expr_with_resolver(
        &expression,
        &DateTimeProfile::default(),
        &|name| match name {
            "First" => Some(Value::Int(1)),
            "Second" => Some(Value::Int(2)),
            "Tail" => Some(Value::Int(3)),
            _ => None,
        },
    )
    .expect("repeat should resolve every value") else {
        panic!("expected array");
    };

    assert_eq!(array.dimensions(), &[(1, 5)]);
    assert_eq!(
        array.elements(),
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
fn zero_repeat_contributes_no_elements_to_nonempty_initializer() {
    let expression = Expr::ArrayInitializer(vec![
        repeat(Value::Int(0), vec![Expr::Literal(Value::Int(99))]),
        Expr::Literal(Value::Int(7)),
    ]);

    let Value::Array(array) =
        eval_const_expr(&expression, &DateTimeProfile::default()).expect("array constant")
    else {
        panic!("expected array");
    };

    assert_eq!(array.dimensions(), &[(1, 1)]);
    assert_eq!(array.elements(), &[Value::Int(7)]);
}

#[test]
fn every_supported_integer_tag_can_supply_a_repeat_count() {
    for count in [
        Value::SInt(1),
        Value::Int(1),
        Value::DInt(1),
        Value::LInt(1),
        Value::USInt(1),
        Value::UInt(1),
        Value::UDInt(1),
        Value::ULInt(1),
    ] {
        let group = repeat(count, vec![Expr::Literal(Value::Int(8))]);
        let Some((count, args)) = array_repeat_group(&group).expect("valid repeat") else {
            panic!("integer literal must be recognized as repeat");
        };
        assert_eq!(count, 1);
        assert_eq!(args.len(), 1);
    }
}

#[test]
fn negative_and_host_unrepresentable_repeat_counts_are_rejected() {
    for count in [Value::LInt(-1), Value::ULInt(u64::MAX)] {
        let expression =
            Expr::ArrayInitializer(vec![repeat(count, vec![Expr::Literal(Value::Int(1))])]);
        assert_unsupported(eval_const_expr(&expression, &DateTimeProfile::default()));
    }
}

#[test]
fn noninteger_call_target_is_not_misclassified_as_repeat_group() {
    let call = Expr::Call {
        target: Box::new(Expr::Name("Factory".into())),
        args: vec![positional(Expr::Literal(Value::Int(1)))],
    };

    assert!(matches!(array_repeat_group(&call), Ok(None)));
    assert_unsupported(eval_const_expr(&call, &DateTimeProfile::default()));
}

#[test]
fn named_repeat_argument_is_rejected_before_element_evaluation() {
    let expression = Expr::ArrayInitializer(vec![Expr::Call {
        target: Box::new(Expr::Literal(Value::Int(2))),
        args: vec![CallArg {
            name: Some("value".into()),
            value: ArgValue::Expr(Expr::Name("MustNotResolve".into())),
        }],
    }]);
    let resolver_called = Cell::new(false);

    let result = eval_const_expr_with_resolver(&expression, &DateTimeProfile::default(), &|_| {
        resolver_called.set(true);
        Some(Value::Int(9))
    });

    assert_unsupported(result);
    assert!(!resolver_called.get());
}

#[test]
fn target_argument_in_repeat_group_is_not_a_constant_value() {
    let expression = Expr::ArrayInitializer(vec![Expr::Call {
        target: Box::new(Expr::Literal(Value::Int(1))),
        args: vec![CallArg {
            name: None,
            value: ArgValue::Target(LValue::Name("x".into())),
        }],
    }]);

    assert_unsupported(eval_const_expr(&expression, &DateTimeProfile::default()));
}

#[test]
fn const_error_conversion_and_display_preserve_runtime_failure() {
    let error = ConstExprError::from(RuntimeError::Overflow);

    assert_eq!(error.to_string(), RuntimeError::Overflow.to_string());
    assert!(matches!(
        error,
        ConstExprError::Runtime(RuntimeError::Overflow)
    ));
    assert_eq!(
        ConstExprError::UnsupportedExpr.to_string(),
        "expression is not a compile-time constant"
    );
}

#[test]
fn size_error_mapping_is_fail_closed_and_exact() {
    assert!(matches!(
        size_error_to_const(SizeOfError::UnknownType),
        ConstExprError::Runtime(RuntimeError::TypeMismatch)
    ));
    assert!(matches!(
        size_error_to_const(SizeOfError::UnsupportedType),
        ConstExprError::Runtime(RuntimeError::TypeMismatch)
    ));
    assert!(matches!(
        size_error_to_const(SizeOfError::Overflow),
        ConstExprError::Runtime(RuntimeError::Overflow)
    ));
}
