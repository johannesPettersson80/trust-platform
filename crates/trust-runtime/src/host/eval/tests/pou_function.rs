use super::common;

use trust_hir::symbols::ParamDirection;
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};
use trust_runtime::error::RuntimeError;
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
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn function_input_default_failure_returns_init_failed() {
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
        return_type: TypeId::INT,
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
            expr: Some(Expr::Literal(Value::Int(1))),
            location: None,
        }],
    };

    let args = vec![CallArg {
        name: Some("Seed".into()),
        value: ArgValue::Expr(Expr::Literal(Value::Int(1))),
    }];
    let err = call_function(&mut ctx, &func, &args)
        .expect_err("missing unsupported input default must fail closed");
    assert_init_failed(err, "NeedsSvc", "Svc");
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn function_return_default_failure_returns_init_failed() {
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

    let err = call_function(&mut ctx, &func, &[])
        .expect_err("unsupported return default must fail closed");
    assert_init_failed(err, "ReturnSvc", "ReturnSvc");
}

#[test]
#[ignore = "red test for runtime-safety fail-closed Phase 1"]
fn function_local_default_failure_returns_init_failed() {
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
        return_type: TypeId::INT,
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
            expr: Some(Expr::Literal(Value::Int(1))),
            location: None,
        }],
    };

    let err = call_function(&mut ctx, &func, &[])
        .expect_err("unsupported local default must fail closed");
    assert_init_failed(err, "LocalSvc", "Svc");
}

fn assert_init_failed(err: RuntimeError, owner: &str, variable: &str) {
    match err {
        RuntimeError::InitFailed {
            owner: actual_owner,
            variable: actual_variable,
            ..
        } => {
            assert_eq!(actual_owner, owner);
            assert_eq!(actual_variable, variable);
        }
        other => panic!("expected InitFailed for {owner}.{variable}, got {other:?}"),
    }
}
