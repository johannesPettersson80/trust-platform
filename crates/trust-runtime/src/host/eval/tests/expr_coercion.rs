use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::{eval_expr, expr::Expr, ops::BinaryOp};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn mixed_numeric_ops() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Binary {
        op: BinaryOp::Add,
        left: Box::new(Expr::Literal(Value::Int(1))),
        right: Box::new(Expr::Literal(Value::DInt(2))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::DInt(3));

    let expr = Expr::Binary {
        op: BinaryOp::Add,
        left: Box::new(Expr::Literal(Value::UInt(2))),
        right: Box::new(Expr::Literal(Value::Int(3))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::UInt(5));

    let expr = Expr::Binary {
        op: BinaryOp::Mul,
        left: Box::new(Expr::Literal(Value::LReal(1.5))),
        right: Box::new(Expr::Literal(Value::Real(2.0))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::LReal(3.0));

    let expr = Expr::Binary {
        op: BinaryOp::Div,
        left: Box::new(Expr::Literal(Value::Real(5.0))),
        right: Box::new(Expr::Literal(Value::Int(2))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Real(2.5));

    let expr = Expr::Binary {
        op: BinaryOp::Lt,
        left: Box::new(Expr::Literal(Value::Int(2))),
        right: Box::new(Expr::Literal(Value::Real(2.5))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Bool(true));

    let expr = Expr::Binary {
        op: BinaryOp::Eq,
        left: Box::new(Expr::Literal(Value::Int(5))),
        right: Box::new(Expr::Literal(Value::DInt(5))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Bool(true));
}
