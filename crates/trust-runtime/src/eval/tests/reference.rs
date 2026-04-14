use super::common;

use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_runtime::eval::eval_expr;
use trust_runtime::eval::expr::{write_lvalue, Expr, LValue};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::{default_value_for_type_id, DateTimeProfile, Value};

#[test]
fn default_null_reference() {
    let mut registry = TypeRegistry::new();
    let profile = DateTimeProfile::default();

    let ref_id = registry.register_reference(TypeId::INT);
    let value = default_value_for_type_id(ref_id, &registry, &profile).unwrap();

    assert_eq!(value, Value::Reference(None));
}

#[test]
fn ref_and_deref() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::Int(5));
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let ref_expr = Expr::Ref(LValue::Name("x".into()));
    let deref_expr = Expr::Deref(Box::new(ref_expr.clone()));

    let value = eval_expr(&mut ctx, &deref_expr).unwrap();
    assert_eq!(value, Value::Int(5));

    let target = LValue::Deref(Box::new(ref_expr));
    write_lvalue(&mut ctx, &target, Value::Int(9)).unwrap();
    let updated = eval_expr(&mut ctx, &Expr::Name("x".into())).unwrap();
    assert_eq!(updated, Value::Int(9));
}
