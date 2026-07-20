use super::common;

use trust_hir::symbols::ParamDirection;
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};
use trust_runtime::eval::{
    call_function, expr::Expr, ops::BinaryOp, stmt::Stmt, ArgValue, CallArg, FunctionDef, Param,
    VarDef,
};
use trust_runtime::memory::VariableStorage;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;
use trust_runtime::RetainPolicy;

#[test]
fn call_function_exec() {
    let registry = TypeRegistry::new();
    let mut storage = VariableStorage::new();
    let stdlib = StandardLibrary::new();
    let mut ctx = common::make_context(&mut storage, &registry);
    ctx.stdlib = Some(&stdlib);

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

#[test]
fn function_input_interface_defaults_to_null() {
    let mut registry = TypeRegistry::new();
    let interface = registry.register(
        "I_Svc",
        Type::Interface {
            name: "I_Svc".into(),
        },
    );
    let mut storage = VariableStorage::new();
    let stdlib = StandardLibrary::new();
    let mut ctx = common::make_context(&mut storage, &registry);
    ctx.stdlib = Some(&stdlib);

    let func = FunctionDef {
        name: "NeedsSvc".into(),
        return_type: interface,
        params: vec![
            Param {
                name: "Seed".into(),
                type_id: TypeId::INT,
                direction: ParamDirection::In,
                address: None,
                default: None,
            },
            Param {
                name: "Svc".into(),
                type_id: interface,
                direction: ParamDirection::In,
                address: None,
                default: None,
            },
        ],
        locals: Vec::new(),
        static_locals: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Return {
            expr: Some(Expr::Name("Svc".into())),
            location: None,
        }],
    };

    let args = vec![CallArg {
        name: Some("Seed".into()),
        value: ArgValue::Expr(Expr::Literal(Value::Int(1))),
    }];
    let result = call_function(&mut ctx, &func, &args).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn function_interface_return_defaults_to_null() {
    let mut registry = TypeRegistry::new();
    let interface = registry.register(
        "I_Svc",
        Type::Interface {
            name: "I_Svc".into(),
        },
    );
    let mut storage = VariableStorage::new();
    let stdlib = StandardLibrary::new();
    let mut ctx = common::make_context(&mut storage, &registry);
    ctx.stdlib = Some(&stdlib);

    let func = FunctionDef {
        name: "ReturnSvc".into(),
        return_type: interface,
        params: Vec::new(),
        locals: Vec::new(),
        static_locals: Vec::new(),
        using: Vec::new(),
        body: Vec::new(),
    };

    let result = call_function(&mut ctx, &func, &[]).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn function_local_interface_defaults_to_null() {
    let mut registry = TypeRegistry::new();
    let interface = registry.register(
        "I_Svc",
        Type::Interface {
            name: "I_Svc".into(),
        },
    );
    let mut storage = VariableStorage::new();
    let stdlib = StandardLibrary::new();
    let mut ctx = common::make_context(&mut storage, &registry);
    ctx.stdlib = Some(&stdlib);

    let func = FunctionDef {
        name: "LocalSvc".into(),
        return_type: interface,
        params: Vec::new(),
        locals: vec![VarDef {
            name: "Svc".into(),
            type_id: interface,
            initializer: None,
            retain: RetainPolicy::Unspecified,
            static_storage: false,
            external: false,
            constant: false,
            address: None,
        }],
        static_locals: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Return {
            expr: Some(Expr::Name("Svc".into())),
            location: None,
        }],
    };

    let result = call_function(&mut ctx, &func, &[]).unwrap();
    assert_eq!(result, Value::Null);
}
