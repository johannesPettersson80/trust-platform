use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::{eval_expr, expr::Expr, ops::BinaryOp};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn precedence() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Binary {
        op: BinaryOp::Add,
        left: Box::new(Expr::Literal(Value::Int(1))),
        right: Box::new(Expr::Binary {
            op: BinaryOp::Mul,
            left: Box::new(Expr::Literal(Value::Int(2))),
            right: Box::new(Expr::Literal(Value::Int(3))),
        }),
    };

    let value = eval_expr(&mut ctx, &expr).unwrap();
    assert_eq!(value, Value::Int(7));
}

#[test]
fn power_operator() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Binary {
        op: BinaryOp::Pow,
        left: Box::new(Expr::Literal(Value::Int(2))),
        right: Box::new(Expr::Literal(Value::Int(3))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Int(8));

    let expr = Expr::Binary {
        op: BinaryOp::Pow,
        left: Box::new(Expr::Literal(Value::LReal(2.0))),
        right: Box::new(Expr::Literal(Value::Real(3.0))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::LReal(8.0));
}

#[test]
fn bitwise_ops() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Binary {
        op: BinaryOp::And,
        left: Box::new(Expr::Literal(Value::Byte(0xF0))),
        right: Box::new(Expr::Literal(Value::Byte(0x0F))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Byte(0x00));

    let expr = Expr::Binary {
        op: BinaryOp::Xor,
        left: Box::new(Expr::Literal(Value::Word(0x00FF))),
        right: Box::new(Expr::Literal(Value::Word(0x0F00))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Word(0x0FFF));

    let expr = Expr::Unary {
        op: trust_runtime::eval::ops::UnaryOp::Not,
        expr: Box::new(Expr::Literal(Value::Byte(0xF0))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Byte(0x0F));
}

#[test]
fn string_and_bool_compare() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Binary {
        op: BinaryOp::Lt,
        left: Box::new(Expr::Literal(Value::String("A".into()))),
        right: Box::new(Expr::Literal(Value::String("B".into()))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Bool(true));

    let expr = Expr::Binary {
        op: BinaryOp::Ge,
        left: Box::new(Expr::Literal(Value::Bool(true))),
        right: Box::new(Expr::Literal(Value::Bool(false))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Bool(true));
}
