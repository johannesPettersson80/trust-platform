use super::common;

use trust_hir::symbols::ParamDirection;
use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_runtime::eval::{
    call_function, expr::Expr, ops::BinaryOp, stmt::Stmt, ArgValue, CallArg, FunctionDef, Param,
};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn call_function_exec() {
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let func = FunctionDef {
        name: "AddOne".into(),
        return_type: TypeId::INT,
        params: vec![Param {
            name: "x".into(),
            type_id: TypeId::INT,
            direction: ParamDirection::In,
            address: None,
            default: None,
        }],
        locals: Vec::new(),
        static_locals: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Return {
            expr: Some(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("x".into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            }),
            location: None,
        }],
    };

    let args = vec![CallArg {
        name: Some("x".into()),
        value: ArgValue::Expr(Expr::Literal(Value::Int(5))),
    }];

    let result = call_function(&mut ctx, &func, &args).unwrap();
    assert_eq!(result, Value::Int(6));
}
