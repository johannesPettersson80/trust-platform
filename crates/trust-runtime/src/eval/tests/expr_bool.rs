use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::{eval_expr, expr::Expr, ops::BinaryOp};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn short_circuit() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Binary {
        op: BinaryOp::And,
        left: Box::new(Expr::Literal(Value::Bool(false))),
        right: Box::new(Expr::Binary {
            op: BinaryOp::Div,
            left: Box::new(Expr::Literal(Value::Int(1))),
            right: Box::new(Expr::Literal(Value::Int(0))),
        }),
    };

    let value = eval_expr(&mut ctx, &expr).unwrap();
    assert_eq!(value, Value::Bool(false));
}
