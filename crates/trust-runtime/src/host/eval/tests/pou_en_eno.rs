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
fn en_eno_semantics() {
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    storage.set_global("count", Value::Int(0));
    storage.set_global("eno", Value::Bool(true));
    let mut ctx = common::make_context(&mut storage, &registry);

    let func = FunctionDef {
        name: "DoWork".into(),
        return_type: TypeId::INT,
        params: vec![
            Param {
                name: "EN".into(),
                type_id: TypeId::BOOL,
                direction: ParamDirection::In,
                address: None,
                default: None,
            },
            Param {
                name: "ENO".into(),
                type_id: TypeId::BOOL,
                direction: ParamDirection::Out,
                address: None,
                default: None,
            },
        ],
        locals: Vec::new(),
        static_locals: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("count".into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("count".into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            },
            location: None,
        }],
    };

    let args = vec![
        CallArg {
            name: Some("EN".into()),
            value: ArgValue::Expr(Expr::Literal(Value::Bool(false))),
        },
        CallArg {
            name: Some("ENO".into()),
            value: ArgValue::Target(LValue::Name("eno".into())),
        },
    ];

    let _ = call_function(&mut ctx, &func, &args).unwrap();
    assert_eq!(storage.get_global("count"), Some(&Value::Int(0)));
    assert_eq!(storage.get_global("eno"), Some(&Value::Bool(false)));
}
