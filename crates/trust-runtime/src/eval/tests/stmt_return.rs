use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::expr::Expr;
use trust_runtime::eval::stmt::{exec_stmt, Stmt, StmtResult};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn default_result() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let stmt = Stmt::Return {
        expr: Some(Expr::Literal(Value::Int(4))),
        location: None,
    };
    let result = exec_stmt(&mut ctx, &stmt).unwrap();
    assert_eq!(result, StmtResult::Return(Some(Value::Int(4))));
}
