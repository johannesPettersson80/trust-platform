use super::common;

use crate::eval::expr::write_lvalue;
use trust_hir::types::TypeRegistry;
use trust_runtime::error::RuntimeError;
use trust_runtime::eval::expr::Expr;
use trust_runtime::eval::ops::{apply_binary, apply_unary, BinaryOp, UnaryOp};
use trust_runtime::eval::{eval_expr, expr::LValue};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::{ArrayValue, DateTimeError, DateTimeProfile, DateValue, Value};

#[test]
fn type_errors() {
    assert_eq!(
        apply_unary(UnaryOp::Neg, Value::Bool(true)),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn neg_overflow_returns_runtime_error() {
    assert_eq!(
        apply_unary(UnaryOp::Neg, Value::SInt(i8::MIN)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        apply_unary(UnaryOp::Neg, Value::Int(i16::MIN)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        apply_unary(UnaryOp::Neg, Value::DInt(i32::MIN)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        apply_unary(UnaryOp::Neg, Value::LInt(i64::MIN)),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn div_overflow() {
    assert_eq!(
        apply_binary(
            BinaryOp::Div,
            Value::Int(4),
            Value::Int(0),
            &DateTimeProfile::default(),
        ),
        Err(RuntimeError::DivisionByZero)
    );
    assert_eq!(
        apply_binary(
            BinaryOp::Add,
            Value::Int(i16::MAX),
            Value::Int(1),
            &DateTimeProfile::default(),
        ),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn index_and_null_ref() {
    let mut storage = VariableStorage::new();
    storage.set_global(
        "arr",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(vec![Value::Int(1), Value::Int(2)], vec![(1, 2)])
                .unwrap(),
        )),
    );
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Index {
        target: Box::new(Expr::Name("arr".into())),
        indices: vec![Expr::Literal(Value::Int(3))],
    };
    let err = eval_expr(&mut ctx, &expr).unwrap_err();
    assert!(matches!(err, RuntimeError::IndexOutOfBounds { .. }));

    let null_ref = Expr::Deref(Box::new(Expr::Literal(Value::Reference(None))));
    let err = eval_expr(&mut ctx, &null_ref).unwrap_err();
    assert_eq!(err, RuntimeError::NullReference);

    let bad_target = Expr::Ref(LValue::Name("missing".into()));
    let err = eval_expr(&mut ctx, &bad_target).unwrap_err();
    assert_eq!(err, RuntimeError::UndefinedVariable("missing".into()));
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn evaluator_unknown_assignment_fails_without_creating_global() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let err = write_lvalue(&mut ctx, &LValue::Name("missing".into()), Value::DInt(7))
        .expect_err("unknown evaluator assignment must fail");

    assert_eq!(err, RuntimeError::UndefinedVariable("missing".into()));
    assert!(ctx.storage.get_global("missing").is_none());
}

#[test]
fn datetime_range() {
    let err = DateValue::try_from_ticks(i128::from(i64::MAX) + 1)
        .map_err(RuntimeError::from)
        .unwrap_err();
    assert_eq!(err, RuntimeError::DateTimeRange(DateTimeError::OutOfRange));
}
