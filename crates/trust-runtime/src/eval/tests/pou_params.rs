use super::common;

use trust_hir::symbols::ParamDirection;
use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::ops::BinaryOp;
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::eval::{call_function, ArgValue, CallArg, FunctionDef, Param};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn param_binding() {
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    storage.set_global("out", Value::Int(0));
    storage.set_global("inout", Value::Int(3));
    let mut ctx = common::make_context(&mut storage, &registry);

    let func = FunctionDef {
        name: "F".into(),
        return_type: TypeId::INT,
        params: vec![
            Param {
                name: "a".into(),
                type_id: TypeId::INT,
                direction: ParamDirection::In,
                address: None,
                default: None,
            },
            Param {
                name: "b".into(),
                type_id: TypeId::INT,
                direction: ParamDirection::Out,
                address: None,
                default: None,
            },
            Param {
                name: "c".into(),
                type_id: TypeId::INT,
                direction: ParamDirection::InOut,
                address: None,
                default: None,
            },
        ],
        locals: Vec::new(),
        static_locals: Vec::new(),
        using: Vec::new(),
        body: vec![
            Stmt::Assign {
                target: LValue::Name("b".into()),
                value: Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Name("a".into())),
                    right: Box::new(Expr::Name("c".into())),
                },
                location: None,
            },
            Stmt::Assign {
                target: LValue::Name("c".into()),
                value: Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Name("c".into())),
                    right: Box::new(Expr::Literal(Value::Int(1))),
                },
                location: None,
            },
            Stmt::Return {
                expr: Some(Expr::Name("a".into())),
                location: None,
            },
        ],
    };

    let args = vec![
        CallArg {
            name: Some("a".into()),
            value: ArgValue::Expr(Expr::Literal(Value::Int(2))),
        },
        CallArg {
            name: Some("b".into()),
            value: ArgValue::Target(LValue::Name("out".into())),
        },
        CallArg {
            name: Some("c".into()),
            value: ArgValue::Target(LValue::Name("inout".into())),
        },
    ];

    let result = call_function(&mut ctx, &func, &args).unwrap();
    assert_eq!(result, Value::Int(2));
    assert_eq!(storage.get_global("out"), Some(&Value::Int(5)));
    assert_eq!(storage.get_global("inout"), Some(&Value::Int(4)));
}
