use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::stmt::{exec_stmt, Stmt};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn assignment() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::Int(0));
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let stmt = Stmt::Assign {
        target: LValue::Name("x".into()),
        value: Expr::Literal(Value::Int(2)),
        location: None,
    };

    let result = exec_stmt(&mut ctx, &stmt).unwrap();
    assert!(matches!(
        result,
        trust_runtime::eval::stmt::StmtResult::Continue
    ));
    assert_eq!(storage.get_global("x"), Some(&Value::Int(2)));
}
