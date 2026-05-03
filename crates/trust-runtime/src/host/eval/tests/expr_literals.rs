use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::{eval_expr, expr::Expr};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn literal_eval() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Literal(Value::Int(42));
    let value = eval_expr(&mut ctx, &expr).unwrap();
    assert_eq!(value, Value::Int(42));
}

#[test]
fn name_ref_eval() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::Int(7));

    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Name("x".into());
    let value = eval_expr(&mut ctx, &expr).unwrap();
    assert_eq!(value, Value::Int(7));
}
